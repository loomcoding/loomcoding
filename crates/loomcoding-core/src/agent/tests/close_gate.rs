//! Política de cierre del turno: reinyección, tope, cancelación, plazo, pánico y acotado de textos.
//!
//! Los plazos se miden con reloj simulado (`tokio::time::pause`), no con espera real.

use super::*;
use crate::agent::close_gate::{consult, CloseDecision, CloseGateCtx, TurnCloseGate};

/// Puerta de prueba: responde con `guion` (el último valor se repite) y cuenta sus consultas.
/// Si `cancelar` está puesto, cancela el turno durante la consulta.
struct PuertaDeGuion {
    guion: Vec<CloseDecision>,
    llamadas: AtomicUsize,
    cancelar: Option<CancellationToken>,
}

impl PuertaDeGuion {
    fn guion(guion: Vec<CloseDecision>) -> Arc<Self> {
        Arc::new(Self {
            guion,
            llamadas: AtomicUsize::new(0),
            cancelar: None,
        })
    }
}

#[async_trait]
impl TurnCloseGate for PuertaDeGuion {
    async fn check(&self, _ctx: &CloseGateCtx, _attempt: u32) -> CloseDecision {
        let i = self.llamadas.fetch_add(1, Ordering::SeqCst);
        if let Some(cancel) = &self.cancelar {
            cancel.cancel();
        }
        self.guion[i.min(self.guion.len() - 1)].clone()
    }
}

/// Puerta que entra en pánico al consultarse.
struct PuertaQueEntraEnPanico;

#[async_trait]
impl TurnCloseGate for PuertaQueEntraEnPanico {
    async fn check(&self, _ctx: &CloseGateCtx, _attempt: u32) -> CloseDecision {
        panic!("fallo simulado de la política de cierre");
    }
}

/// Puerta que tarda diez segundos (más que el plazo) antes de pedir reinyección.
struct PuertaLenta;

#[async_trait]
impl TurnCloseGate for PuertaLenta {
    async fn check(&self, _ctx: &CloseGateCtx, _attempt: u32) -> CloseDecision {
        tokio::time::sleep(Duration::from_secs(10)).await;
        CloseDecision::Reinject {
            message: "tarde".into(),
            notice: None,
        }
    }
}

fn texto(t: &str) -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta(t.into()),
        StreamChunk::Done(FinishReason::Stop),
    ]
}

fn reinyeccion(message: &str, notice: Option<&str>) -> CloseDecision {
    CloseDecision::Reinject {
        message: message.into(),
        notice: notice.map(str::to_string),
    }
}

fn cierre() -> CloseDecision {
    CloseDecision::Close { notice: None }
}

/// Contexto para llamar a `consult` directamente, con un almacén temporal.
async fn contexto() -> (tempfile::TempDir, CloseGateCtx) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("c.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let session = store.create_session("/p").await.unwrap();
    let ctx = CloseGateCtx {
        session,
        cwd: dir.path().to_path_buf(),
        store,
    };
    (dir, ctx)
}

/// Turno del agente con las respuestas `normales` del proveedor y sin herramientas.
async fn turno(normales: Vec<Vec<StreamChunk>>) -> EntornoDeTurno {
    entorno_de_turno(
        proveedor_de("", false, normales),
        catalogo_de_ventana(100_000),
        &[],
        ToolRegistry::new(),
    )
    .await
}

fn avisos(rx: &mut broadcast::Receiver<LiveEvent>) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(evento) = rx.try_recv() {
        if let LiveEvent::RuntimeNotice { message } = evento {
            out.push(message);
        }
    }
    out
}

#[tokio::test]
async fn cerrar_no_pide_otra_respuesta_al_modelo() {
    let gate = PuertaDeGuion::guion(vec![cierre()]);
    let e = turno(vec![texto("primera")]).await;
    let agente = e.agente.with_close_gate(gate.clone());
    let out = agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(out.final_text, "primera");
    assert_eq!(e.proveedor.seen.lock().unwrap().len(), 1);
    assert_eq!(gate.llamadas.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn reinyectar_persiste_el_mensaje_interno_y_el_modelo_lo_recibe() {
    let gate = PuertaDeGuion::guion(vec![
        reinyeccion("MENSAJE_INTERNO", Some("aviso de prueba")),
        cierre(),
    ]);
    let e = turno(vec![texto("primera"), texto("segunda")]).await;
    let agente = e.agente.with_close_gate(gate);
    let mut rx = agente.bus.subscribe();
    let out = agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(out.final_text, "segunda");
    {
        let peticiones = e.proveedor.seen.lock().unwrap();
        assert_eq!(
            peticiones.len(),
            2,
            "tras reinyectar hay una segunda petición"
        );
        assert!(
            peticiones[1].messages.iter().any(
                |m| m.origin == MessageOrigin::Internal && m.text().contains("MENSAJE_INTERNO")
            ),
            "la segunda petición debe llevar el mensaje reinyectado"
        );
    }

    let persistidos = e.store.messages(e.sid).await.unwrap();
    assert!(
        persistidos
            .iter()
            .any(|m| m.role == Role::User && m.text().contains("MENSAJE_INTERNO")),
        "el mensaje reinyectado debe quedar persistido"
    );
    assert!(avisos(&mut rx).contains(&"aviso de prueba".to_string()));
}

#[tokio::test]
async fn el_tope_de_reinyecciones_corta_aunque_la_politica_insista() {
    let gate = PuertaDeGuion::guion(vec![reinyeccion("otra vez", None)]);
    let e = turno(vec![
        texto("uno"),
        texto("dos"),
        texto("tres"),
        texto("cuatro"),
    ])
    .await;
    let agente = e.agente.with_close_gate(gate.clone());
    let out = agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();

    let reintentos = MAX_CLOSE_REINJECTIONS as usize;
    assert_eq!(e.proveedor.seen.lock().unwrap().len(), 1 + reintentos);
    assert_eq!(gate.llamadas.load(Ordering::SeqCst), reintentos);
    assert_eq!(out.final_text, "cuatro");
}

#[tokio::test]
async fn la_cancelacion_gana_y_no_reinyecta() {
    let cancel = CancellationToken::new();
    let gate = Arc::new(PuertaDeGuion {
        guion: vec![reinyeccion("no debe llegar", None)],
        llamadas: AtomicUsize::new(0),
        cancelar: Some(cancel.clone()),
    });
    let e = turno(vec![texto("primera"), texto("segunda")]).await;
    let agente = e.agente.with_close_gate(gate);
    agente.run(e.sid, "hola".into(), cancel).await.unwrap();

    assert_eq!(e.proveedor.seen.lock().unwrap().len(), 1);
    let persistidos = e.store.messages(e.sid).await.unwrap();
    assert!(!persistidos
        .iter()
        .any(|m| m.text().contains("no debe llegar")));
}

#[tokio::test]
async fn una_politica_que_entra_en_panico_no_rompe_el_turno() {
    let e = turno(vec![texto("primera")]).await;
    let agente = e.agente.with_close_gate(Arc::new(PuertaQueEntraEnPanico));
    let out = agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .expect("el turno termina bien aunque la política falle");
    assert_eq!(out.final_text, "primera");
    assert_eq!(e.proveedor.seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn consulta_con_la_cancelacion_ya_hecha_cierra_sin_llamar_a_la_politica() {
    let (_dir, ctx) = contexto().await;
    let gate = PuertaDeGuion::guion(vec![reinyeccion("no", None)]);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let decision = consult(gate.clone(), ctx, 0, &cancel).await;
    assert!(matches!(decision, CloseDecision::Close { .. }));
    assert_eq!(gate.llamadas.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn consulta_en_el_tope_cierra_sin_llamar_a_la_politica() {
    let (_dir, ctx) = contexto().await;
    let gate = PuertaDeGuion::guion(vec![reinyeccion("no", None)]);
    let decision = consult(
        gate.clone(),
        ctx,
        MAX_CLOSE_REINJECTIONS,
        &CancellationToken::new(),
    )
    .await;
    assert!(matches!(decision, CloseDecision::Close { .. }));
    assert_eq!(gate.llamadas.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn una_politica_que_tarda_mas_del_plazo_cierra_el_turno() {
    let (_dir, ctx) = contexto().await;
    // El almacén ya existe con el reloj real; a partir de aquí el tiempo es simulado.
    tokio::time::pause();
    let decision = consult(Arc::new(PuertaLenta), ctx, 0, &CancellationToken::new()).await;
    assert!(
        matches!(decision, CloseDecision::Close { notice: Some(_) }),
        "vencido el plazo, el turno se cierra con un aviso"
    );
}

#[tokio::test]
async fn consulta_con_panico_cierra_sin_propagarlo() {
    let (_dir, ctx) = contexto().await;
    let decision = consult(
        Arc::new(PuertaQueEntraEnPanico),
        ctx,
        0,
        &CancellationToken::new(),
    )
    .await;
    assert!(matches!(decision, CloseDecision::Close { notice: Some(_) }));
}

#[tokio::test]
async fn mensaje_y_aviso_enormes_se_acotan() {
    let (_dir, ctx) = contexto().await;
    let gate = PuertaDeGuion::guion(vec![CloseDecision::Reinject {
        message: "m".repeat(200_000),
        notice: Some("n".repeat(200_000)),
    }]);
    let decision = consult(gate, ctx, 0, &CancellationToken::new()).await;
    let CloseDecision::Reinject { message, notice } = decision else {
        panic!("debía reinyectar");
    };
    assert!(message.len() <= MAX_CLOSE_MESSAGE_BYTES);
    assert!(notice.unwrap().len() <= MAX_NOTICE_BYTES);
}

#[tokio::test]
async fn sin_politica_el_turno_cierra_como_siempre() {
    let e = turno(vec![texto("primera")]).await;
    let out = e
        .agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(out.final_text, "primera");
    assert_eq!(e.proveedor.seen.lock().unwrap().len(), 1);
}
