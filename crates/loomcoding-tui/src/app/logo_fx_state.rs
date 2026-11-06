//! Estado de los efectos del logo de cabecera (`/logofx`): el planificador, la preferencia y el
//! fotograma que el render debe pintar.
//!
//! El render solo lee `logo_fx_frame`; nunca consulta el reloj. El reloj entra por
//! `tick_logo_fx`, que el bucle llama en cada pasada con `Instant::now()`.

use super::*;
use crate::prefs::{LOGO_FX_MAX_SECS, LOGO_FX_MIN_SECS};
use loomcoding_cloth::widgets::logo_fx::{LogoFx, LogoFxState};
use loomcoding_cloth::widgets::theme::ColorDepth;
use loomcoding_cloth::widgets::LayoutStyle;

/// Intervalo por defecto entre ráfagas, en segundos (spec del menú izquierdo).
pub const LOGO_FX_DEFAULT_SECS: u64 = 20;

/// Planificador de arranque. La semilla sale del reloj del sistema solo en producción; en
/// pruebas es fija, para que nada dependa del momento en que corren.
pub(super) fn new_logo_fx() -> LogoFx {
    let seed = if cfg!(test) {
        0x5EED_0000_0001
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
    };
    LogoFx::new(
        Duration::from_secs(LOGO_FX_DEFAULT_SECS),
        seed,
        Instant::now(),
    )
}

/// Cierto si el entorno pide sin color (`NO_COLOR`). Se lee una vez, al construir `App`.
pub(super) fn no_color_from_env() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

/// Semilla de cada ráfaga: distinta por número de ráfaga, para que no repitan el mismo patrón.
fn seed_of(burst: u64) -> u64 {
    burst.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

impl App {
    /// Avanza el planificador a `now` y fija `logo_fx_frame` para el próximo render. Devuelve
    /// `true` si el fotograma cambió, para que el bucle repinte.
    ///
    /// El planificador avanza siempre, también con el logo oculto. Así su plazo nunca queda
    /// vencido (el bucle no podría dormir sobre él) y, oculto, no necesita reloj.
    pub fn tick_logo_fx(&mut self, now: Instant) -> bool {
        let nuevo = match self.logo_fx.tick(now) {
            LogoFxState::Active {
                effect,
                started,
                ends,
            } if self.logo_fx_visible() => {
                // Cada ráfaga cuenta una vez, la primera vez que se ve.
                if self.logo_fx_last_started != Some(started) {
                    self.logo_fx_last_started = Some(started);
                    self.logo_fx_bursts += 1;
                }
                let total = ends.saturating_duration_since(started).as_secs_f32();
                let transcurrido = now.saturating_duration_since(started).as_secs_f32();
                let progreso = if total > 0.0 {
                    (transcurrido / total).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                Some((effect, progreso, seed_of(self.logo_fx_bursts)))
            }
            _ => None,
        };
        let cambio = nuevo != self.logo_fx_frame;
        self.logo_fx_frame = nuevo;
        cambio
    }

    /// Próximo instante en que el planificador tiene algo que hacer. El bucle despierta ahí si no
    /// hay otro evento; `None` con los efectos apagados.
    pub fn logo_fx_deadline(&self) -> Option<Instant> {
        self.logo_fx.deadline()
    }

    /// Cierto si hay un overlay abierto que tapa el menú izquierdo.
    pub fn overlay_open(&self) -> bool {
        self.permission.is_some()
            || self.question.is_some()
            || self.session_picker_open
            || self.queue_open
            || self.session_loading
            || self.settings_open
            || self.temas_open
            || self.help_open
            || self.agents_open
            || self.a2a_open
            || self.tools_open
    }

    /// Cierto si el logo puede moverse ahora: preferencia encendida, layout `Workspace`, menú
    /// izquierdo pintado en el último fotograma, ningún overlay encima y color disponible.
    fn logo_fx_visible(&self) -> bool {
        self.logo_fx_on
            && self.theme.look.layout == LayoutStyle::Workspace
            && self
                .pane_rects
                .iter()
                .any(|(foco, _)| *foco == Focus::Sidebar)
            && !self.overlay_open()
            && self.theme.depth != ColorDepth::NoColor
            && !self.logo_fx_sin_color
    }

    /// Enciende o apaga los efectos del logo (`/logofx on|off`), los persiste si hay `prefs_path`
    /// y avisa en `slash_feedback` si el guardado falla, sin impedir el cambio.
    pub fn set_logo_fx(&mut self, on: bool) {
        self.logo_fx_on = on;
        self.logo_fx.set_enabled(on, Instant::now());
        self.logo_fx_frame = None;
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_logo_fx(path, on) {
                self.slash_feedback = Some(format!("Logo effects setting not saved: {err}"));
            }
        }
    }

    /// Fija el intervalo entre ráfagas, en segundos, acotado a 5..=600. Lo persiste si hay
    /// `prefs_path` y avisa en `slash_feedback` si el guardado falla.
    pub fn set_logo_fx_interval(&mut self, secs: u64) {
        let secs = secs.clamp(LOGO_FX_MIN_SECS, LOGO_FX_MAX_SECS);
        self.logo_fx
            .set_interval(Duration::from_secs(secs), Instant::now());
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_logo_fx_interval(path, secs) {
                self.slash_feedback = Some(format!("Logo effects interval not saved: {err}"));
            }
        }
    }

    /// Aplica el argumento de `/logofx`. Vacío alterna; `on` y `off` fijan; un número fija el
    /// intervalo en segundos. Cualquier otra cosa avisa del uso sin cambiar nada.
    pub fn logofx_command(&mut self, arg: &str) {
        match arg {
            "" | "on" | "off" => {
                let on = match arg {
                    "on" => true,
                    "off" => false,
                    _ => !self.logo_fx_on,
                };
                self.set_logo_fx(on);
                let texto = if on { "on" } else { "off" };
                self.slash_feedback
                    .get_or_insert_with(|| format!("logo fx: {texto}"));
            }
            _ => match arg.parse::<u64>() {
                Ok(secs) => {
                    self.set_logo_fx_interval(secs);
                    let cada = self.logo_fx.interval().as_secs();
                    self.slash_feedback
                        .get_or_insert_with(|| format!("logo fx: cada {cada} s"));
                }
                Err(_) => {
                    self.slash_feedback = Some("Usage: /logofx [on|off|segundos]".into());
                }
            },
        }
    }

    /// Carga las preferencias del logo desde `path`. Lo que falte o no sea válido se queda como está.
    pub(super) fn load_logo_fx_prefs(&mut self, path: &std::path::Path) {
        let now = Instant::now();
        if let Some(on) = crate::prefs::load_logo_fx(path) {
            self.logo_fx_on = on;
            self.logo_fx.set_enabled(on, now);
        }
        if let Some(secs) = crate::prefs::load_logo_fx_interval(path) {
            self.logo_fx.set_interval(Duration::from_secs(secs), now);
        }
    }
}
