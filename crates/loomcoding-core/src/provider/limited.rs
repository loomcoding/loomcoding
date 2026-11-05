//! Envoltorio de `Provider` con tope de peticiones simultáneas y reintento ante `429`.

use super::{Capabilities, Provider, ProviderRequest, ProviderStream, StreamChunk};
use crate::error::{CoreError, Result};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

/// Reintentos ante `http 429` antes de rendirse (decálogo: todo bucle lleva cota).
pub const MAX_RATE_LIMIT_RETRIES: u32 = 3;
const BASE_BACKOFF: Duration = Duration::from_millis(500);

pub struct LimitedProvider {
    inner: Arc<dyn Provider>,
    permits: Arc<Semaphore>,
    backoff: Duration,
}

impl LimitedProvider {
    /// `max_concurrent == 0` se trata como uno: un semáforo sin permisos bloquearía para siempre.
    pub fn new(inner: Arc<dyn Provider>, max_concurrent: usize) -> Self {
        Self {
            inner,
            permits: Arc::new(Semaphore::new(max_concurrent.max(1))),
            backoff: BASE_BACKOFF,
        }
    }

    #[cfg(test)]
    fn with_backoff(mut self, backoff: Duration) -> Self {
        self.backoff = backoff;
        self
    }
}

/// Flujo que conserva el permiso del semáforo hasta que termina o se suelta.
struct Held {
    inner: ProviderStream,
    _permit: OwnedSemaphorePermit,
}

impl Stream for Held {
    type Item = StreamChunk;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<StreamChunk>> {
        self.inner.as_mut().poll_next(cx)
    }
}

fn is_rate_limited(error: &CoreError) -> bool {
    matches!(error, CoreError::Provider(text) if text.to_ascii_lowercase().contains("http 429"))
}

#[async_trait]
impl Provider for LimitedProvider {
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    async fn stream(
        &self,
        req: ProviderRequest,
        cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        let permit = tokio::select! {
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            permit = self.permits.clone().acquire_owned() => {
                permit.map_err(|_| CoreError::Cancelled)?
            }
        };
        let mut attempt = 0u32;
        loop {
            match self.inner.stream(req.clone(), cancel.clone()).await {
                Ok(inner) => {
                    return Ok(Box::pin(Held {
                        inner,
                        _permit: permit,
                    }))
                }
                Err(error) if is_rate_limited(&error) && attempt < MAX_RATE_LIMIT_RETRIES => {
                    let wait = self.backoff * 2u32.pow(attempt);
                    tracing::warn!(attempt, ?wait, "provider answered 429; backing off");
                    attempt += 1;
                    tokio::select! {
                        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                        _ = tokio::time::sleep(wait) => {}
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::FinishReason;
    use futures::StreamExt;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::sync::Mutex;

    /// Proveedor de prueba: cuenta flujos abiertos a la vez y puede responder antes con 429.
    struct Probe {
        open: AtomicUsize,
        peak: AtomicUsize,
        calls: AtomicUsize,
        rate_limited: Mutex<usize>,
        status: &'static str,
    }

    impl Probe {
        fn new(rate_limited: usize) -> Arc<Self> {
            Self::with_status(rate_limited, "http 429 Too Many Requests")
        }

        fn with_status(rate_limited: usize, status: &'static str) -> Arc<Self> {
            Arc::new(Self {
                open: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                calls: AtomicUsize::new(0),
                rate_limited: Mutex::new(rate_limited),
                status,
            })
        }
    }

    /// Flujo de un solo trozo; mientras existe cuenta como petición abierta.
    struct Counted {
        probe: Arc<Probe>,
        sent: bool,
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            self.probe.open.fetch_sub(1, SeqCst);
        }
    }

    impl Stream for Counted {
        type Item = StreamChunk;
        fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<StreamChunk>> {
            if self.sent {
                return Poll::Ready(None);
            }
            self.sent = true;
            Poll::Ready(Some(StreamChunk::Done(FinishReason::Stop)))
        }
    }

    struct Shared(Arc<Probe>);

    #[async_trait]
    impl Provider for Shared {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                tool_use: true,
                reasoning: false,
                streaming: true,
            }
        }
        async fn stream(
            &self,
            _req: ProviderRequest,
            _cancel: CancellationToken,
        ) -> Result<ProviderStream> {
            let probe = &self.0;
            probe.calls.fetch_add(1, SeqCst);
            {
                let mut left = probe.rate_limited.lock().unwrap();
                if *left > 0 {
                    *left -= 1;
                    return Err(CoreError::Provider(probe.status.into()));
                }
            }
            let active = probe.open.fetch_add(1, SeqCst) + 1;
            probe.peak.fetch_max(active, SeqCst);
            Ok(Box::pin(Counted {
                probe: probe.clone(),
                sent: false,
            }))
        }
    }

    fn request() -> ProviderRequest {
        ProviderRequest {
            model: "m".into(),
            system: String::new(),
            messages: vec![],
            tools: vec![],
            temperature: 0.0,
            max_tokens: 16,
            effort: None,
            programmatic_tool_calling: false,
        }
    }

    fn limited(probe: &Arc<Probe>, max: usize) -> Arc<LimitedProvider> {
        Arc::new(
            LimitedProvider::new(Arc::new(Shared(probe.clone())), max)
                .with_backoff(Duration::from_millis(1)),
        )
    }

    #[tokio::test]
    async fn never_more_than_the_limit_run_at_once() {
        let probe = Probe::new(0);
        let limited = limited(&probe, 2);
        let tareas: Vec<_> = (0..6)
            .map(|_| {
                let limited = limited.clone();
                tokio::spawn(async move {
                    let mut flujo = limited
                        .stream(request(), CancellationToken::new())
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    while flujo.next().await.is_some() {}
                })
            })
            .collect();
        for tarea in tareas {
            tarea.await.unwrap();
        }
        assert_eq!(probe.calls.load(SeqCst), 6);
        assert_eq!(probe.peak.load(SeqCst), 2, "dos a la vez, nunca más");
    }

    #[tokio::test]
    async fn the_permit_is_held_until_the_stream_ends() {
        let probe = Probe::new(0);
        let limited = limited(&probe, 1);
        let first = limited
            .stream(request(), CancellationToken::new())
            .await
            .unwrap();
        let blocked = tokio::time::timeout(
            Duration::from_millis(100),
            limited.stream(request(), CancellationToken::new()),
        )
        .await;
        assert!(
            blocked.is_err(),
            "con el flujo abierto no hay permiso libre"
        );
        drop(first);
        let second = tokio::time::timeout(
            Duration::from_secs(2),
            limited.stream(request(), CancellationToken::new()),
        )
        .await
        .expect("al soltarse el flujo el permiso queda libre");
        assert!(second.is_ok());
    }

    #[tokio::test]
    async fn a_429_is_retried_then_succeeds() {
        let probe = Probe::new(2);
        let limited = limited(&probe, 1);
        let result = limited.stream(request(), CancellationToken::new()).await;
        assert!(result.is_ok());
        assert_eq!(probe.calls.load(SeqCst), 3);
    }

    #[tokio::test]
    async fn a_429_gives_up_after_the_retry_cap() {
        let probe = Probe::new(100);
        let limited = limited(&probe, 1);
        let Err(error) = limited.stream(request(), CancellationToken::new()).await else {
            panic!("debía rendirse");
        };
        assert!(matches!(&error, CoreError::Provider(text) if text.contains("429")));
        assert_eq!(
            probe.calls.load(SeqCst),
            1 + MAX_RATE_LIMIT_RETRIES as usize
        );
    }

    #[tokio::test]
    async fn other_errors_are_not_retried() {
        let probe = Probe::with_status(100, "http 500 Internal Server Error");
        let limited = limited(&probe, 1);
        assert!(limited
            .stream(request(), CancellationToken::new())
            .await
            .is_err());
        assert_eq!(probe.calls.load(SeqCst), 1);
    }

    #[tokio::test]
    async fn a_cancelled_waiter_does_not_hang() {
        let probe = Probe::new(0);
        let limited = limited(&probe, 1);
        let _held = limited
            .stream(request(), CancellationToken::new())
            .await
            .unwrap();
        let cancel = CancellationToken::new();
        let stopper = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            stopper.cancel();
        });
        let result =
            tokio::time::timeout(Duration::from_secs(2), limited.stream(request(), cancel))
                .await
                .expect("la espera cancelada no debe colgarse");
        assert!(matches!(result, Err(CoreError::Cancelled)));
    }
}
