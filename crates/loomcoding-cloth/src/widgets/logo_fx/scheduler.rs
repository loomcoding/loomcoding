//! Planificador de las ráfagas: cuándo empieza una, qué efecto es y cuándo acaba.
//! El reloj entra siempre por parámetro; este módulo no mira el reloj del sistema.

use std::time::{Duration, Instant};

use super::{LogoEffect, Rng};

/// Intervalo mínimo entre ráfagas.
pub const MIN_INTERVAL: Duration = Duration::from_secs(5);
/// Intervalo máximo entre ráfagas.
pub const MAX_INTERVAL: Duration = Duration::from_secs(600);

/// Duración mínima de una ráfaga (salvo la linterna, que dura siempre `FLASHLIGHT_MS`).
const MIN_BURST_MS: u64 = 1_500;
/// Duración máxima de una ráfaga.
const MAX_BURST_MS: u64 = 3_000;
/// Duración fija de la linterna.
const FLASHLIGHT_MS: u64 = 1_500;

/// Estado del logo en un instante dado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoFxState {
    /// Logo quieto.
    Idle,
    /// Ráfaga en curso entre `started` y `ends`.
    Active {
        effect: LogoEffect,
        started: Instant,
        ends: Instant,
    },
}

/// Planificador de ráfagas. Sin ráfaga activa no necesita reloj.
#[derive(Debug, Clone)]
pub struct LogoFx {
    interval: Duration,
    last: Option<LogoEffect>,
    next_at: Instant,
    rng: Rng,
    active: Option<(LogoEffect, Instant, Instant)>,
    enabled: bool,
}

impl LogoFx {
    /// Crea el planificador. El intervalo se acota a `MIN_INTERVAL..=MAX_INTERVAL`
    /// y la primera ráfaga llega a `now + interval`.
    pub fn new(interval: Duration, seed: u64, now: Instant) -> Self {
        let interval = interval.clamp(MIN_INTERVAL, MAX_INTERVAL);
        Self {
            interval,
            last: None,
            next_at: now + interval,
            rng: Rng::new(seed),
            active: None,
            enabled: true,
        }
    }

    /// Enciende o apaga los efectos. Apagar corta la ráfaga en curso.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on {
            self.active = None;
        }
    }

    /// Avanza al instante `now` y devuelve el estado del logo.
    pub fn tick(&mut self, now: Instant) -> LogoFxState {
        if !self.enabled {
            return LogoFxState::Idle;
        }
        if let Some((effect, started, ends)) = self.active {
            if now < ends {
                return LogoFxState::Active {
                    effect,
                    started,
                    ends,
                };
            }
            self.active = None;
            self.next_at = now + self.interval;
            return LogoFxState::Idle;
        }
        if now < self.next_at {
            return LogoFxState::Idle;
        }
        let effect = self.pick();
        let ms = if effect == LogoEffect::Flashlight {
            FLASHLIGHT_MS
        } else {
            let span = MAX_BURST_MS - MIN_BURST_MS + 1;
            MIN_BURST_MS + self.rng.below(span as usize) as u64
        };
        let ends = now + Duration::from_millis(ms);
        self.last = Some(effect);
        self.active = Some((effect, now, ends));
        LogoFxState::Active {
            effect,
            started: now,
            ends,
        }
    }

    /// Elige un efecto distinto del último.
    fn pick(&mut self) -> LogoEffect {
        let candidatos: Vec<LogoEffect> = LogoEffect::ALL
            .into_iter()
            .filter(|&e| Some(e) != self.last)
            .collect();
        candidatos[self.rng.below(candidatos.len())]
    }

    /// Próximo instante con algo que hacer: el fin de la ráfaga o la próxima. `None` si está apagado.
    pub fn deadline(&self) -> Option<Instant> {
        if !self.enabled {
            return None;
        }
        Some(self.active.map_or(self.next_at, |(_, _, ends)| ends))
    }

    /// Cierto solo mientras hay una ráfaga activa.
    pub fn needs_clock(&self) -> bool {
        self.enabled && self.active.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn fx(seed: u64, base: Instant) -> LogoFx {
        LogoFx::new(Duration::from_secs(20), seed, base)
    }

    /// Dispara una ráfaga en su `deadline`, comprueba su fin y la cierra. Devuelve efecto y duración.
    fn one_burst(fx: &mut LogoFx) -> (LogoEffect, Duration) {
        let at = fx.deadline().expect("con efectos encendidos hay deadline");
        let LogoFxState::Active {
            effect,
            started,
            ends,
        } = fx.tick(at)
        else {
            panic!("debía dispararse en {at:?}");
        };
        assert_eq!(
            fx.deadline(),
            Some(ends),
            "en ráfaga, el deadline es su fin"
        );
        assert_eq!(fx.tick(ends), LogoFxState::Idle);
        (effect, ends.duration_since(started))
    }

    #[test]
    fn no_dispara_antes_de_next_at() {
        let t0 = Instant::now();
        let mut f = fx(1, t0);
        assert_eq!(f.tick(t0 + Duration::from_secs(19)), LogoFxState::Idle);
        assert!(!f.needs_clock());
    }

    #[test]
    fn dispara_en_next_at_y_pide_reloj() {
        let t0 = Instant::now();
        let mut f = fx(1, t0);
        let at = t0 + Duration::from_secs(20);
        assert!(matches!(f.tick(at), LogoFxState::Active { .. }));
        assert!(f.needs_clock());
    }

    #[test]
    fn al_acabar_vuelve_a_idle_y_avanza_un_intervalo() {
        let t0 = Instant::now();
        let mut f = fx(1, t0);
        let LogoFxState::Active { ends, .. } = f.tick(t0 + Duration::from_secs(20)) else {
            panic!("debía dispararse");
        };
        assert_eq!(f.tick(ends), LogoFxState::Idle);
        assert!(!f.needs_clock());
        assert_eq!(f.deadline(), Some(ends + Duration::from_secs(20)));
    }

    #[test]
    fn nunca_el_mismo_efecto_dos_veces_seguidas() {
        let t0 = Instant::now();
        for seed in [1u64, 2, 42, 0xDEAD_BEEF] {
            let mut f = fx(seed, t0);
            let mut previo: Option<LogoEffect> = None;
            for _ in 0..200 {
                let (efecto, _) = one_burst(&mut f);
                assert_ne!(Some(efecto), previo, "semilla {seed}");
                previo = Some(efecto);
            }
        }
    }

    #[test]
    fn duracion_en_rango_y_linterna_exacta() {
        let t0 = Instant::now();
        for seed in [3u64, 4, 5] {
            let mut f = fx(seed, t0);
            for _ in 0..200 {
                let (efecto, dur) = one_burst(&mut f);
                if efecto == LogoEffect::Flashlight {
                    assert_eq!(dur, Duration::from_millis(1_500));
                } else {
                    assert!(
                        dur >= Duration::from_millis(1_500) && dur <= Duration::from_millis(3_000),
                        "{efecto:?} dura {dur:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn deadline_es_next_at_en_reposo_y_fin_en_rafaga() {
        let t0 = Instant::now();
        let mut f = fx(9, t0);
        assert_eq!(f.deadline(), Some(t0 + Duration::from_secs(20)));
        let LogoFxState::Active { ends, .. } = f.tick(t0 + Duration::from_secs(20)) else {
            panic!("debía dispararse");
        };
        assert_eq!(f.deadline(), Some(ends));
    }

    #[test]
    fn apagado_no_tiene_deadline_ni_dispara() {
        let t0 = Instant::now();
        let mut f = fx(9, t0);
        f.set_enabled(false);
        assert_eq!(f.deadline(), None);
        assert_eq!(
            f.tick(t0 + Duration::from_secs(60 * 60 * 24)),
            LogoFxState::Idle
        );
        assert!(!f.needs_clock());
    }

    #[test]
    fn apagar_en_rafaga_la_corta() {
        let t0 = Instant::now();
        let mut f = fx(9, t0);
        let at = t0 + Duration::from_secs(20);
        assert!(matches!(f.tick(at), LogoFxState::Active { .. }));
        f.set_enabled(false);
        assert!(!f.needs_clock());
        assert_eq!(f.deadline(), None);
    }

    #[test]
    fn intervalo_acotado_a_5_y_600_segundos() {
        let t0 = Instant::now();
        let corto = LogoFx::new(Duration::from_secs(1), 1, t0);
        assert_eq!(corto.deadline(), Some(t0 + Duration::from_secs(5)));
        let largo = LogoFx::new(Duration::from_secs(10_000), 1, t0);
        assert_eq!(largo.deadline(), Some(t0 + Duration::from_secs(600)));
        assert_eq!(MIN_INTERVAL, Duration::from_secs(5));
        assert_eq!(MAX_INTERVAL, Duration::from_secs(600));
    }

    #[test]
    fn misma_semilla_misma_secuencia() {
        let t0 = Instant::now();
        let mut a = fx(77, t0);
        let mut b = fx(77, t0);
        let sa: Vec<_> = (0..50).map(|_| one_burst(&mut a)).collect();
        let sb: Vec<_> = (0..50).map(|_| one_burst(&mut b)).collect();
        assert_eq!(sa, sb);
    }

    #[test]
    fn los_siete_efectos_aparecen_en_200_rafagas() {
        let t0 = Instant::now();
        let mut f = fx(2024, t0);
        let vistos: HashSet<LogoEffect> = (0..200).map(|_| one_burst(&mut f).0).collect();
        assert_eq!(vistos.len(), LogoEffect::ALL.len(), "{vistos:?}");
    }
}
