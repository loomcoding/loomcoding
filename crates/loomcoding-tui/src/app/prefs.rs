//! Tema, ratón, avisos del proveedor y filtro de comandos con barra.

use super::*;

impl App {
    /// Selecciona el look inicial de la TUI de Loomcoding.
    pub fn use_product_default_view(&mut self) {
        self.set_theme(loomcoding_cloth::widgets::ThemePreset::Crush);
    }

    /// Pasa al siguiente preset de la rotación (`/theme`).
    pub fn cycle_theme(&mut self) {
        use loomcoding_cloth::widgets::ThemePreset;
        let presets = ThemePreset::ALL;
        let current = presets
            .iter()
            .position(|preset| *preset == self.theme_preset)
            .unwrap_or(0);
        self.set_theme(presets[(current + 1) % presets.len()]);
    }

    /// Fija el tema, lo persiste si hay `prefs_path` y reaplica el look a lo ya dibujado.
    /// Un fallo al guardar no impide el cambio: se avisa en `slash_feedback`.
    pub fn set_theme(&mut self, preset: loomcoding_cloth::widgets::ThemePreset) {
        self.apply_theme(preset);
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_theme(path, preset) {
                self.slash_feedback = Some(format!("Theme not saved: {err}"));
            }
        }
    }

    /// Fija la disposición (`/layout`), la persiste si hay `prefs_path` y la aplica
    /// al look actual. Un fallo al guardar no impide el cambio: se avisa en `slash_feedback`.
    pub fn set_layout(&mut self, layout: loomcoding_cloth::widgets::LayoutStyle) {
        self.layout_override = Some(layout);
        self.theme.look.layout = layout;
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_layout(path, layout) {
                self.slash_feedback = Some(format!("Layout not saved: {err}"));
            }
        }
    }

    /// Fija el paquete de estilo (`classic`/`flat`, desde `/ui`), lo persiste si hay `prefs_path`
    /// y lo aplica al look actual sin tocar el layout. Un fallo al guardar no impide el cambio:
    /// se avisa en `slash_feedback`.
    pub fn set_style(&mut self, bundle: loomcoding_cloth::widgets::StyleBundle) {
        self.style_override = Some(bundle);
        self.theme.look = self.theme.look.with_bundle(bundle);
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_style(path, bundle) {
                self.slash_feedback = Some(format!("Style not saved: {err}"));
            }
        }
    }

    /// Fija el fichero de preferencias y aplica el tema guardado en él, si lo hay.
    /// No vuelve a escribir el fichero al aplicarlo.
    pub fn set_prefs_path(&mut self, path: std::path::PathBuf) {
        if let Some(preset) = crate::prefs::load_theme(&path) {
            self.apply_theme(preset);
        }
        // La disposición es independiente del color: un layout guardado manda
        // sobre el del preset, también si se ha cargado un tema.
        if let Some(layout) = crate::prefs::load_layout(&path) {
            self.layout_override = Some(layout);
            self.theme.look.layout = layout;
        }
        // El estilo guardado sustituye al del preset y no toca el layout.
        if let Some(bundle) = crate::prefs::load_style(&path) {
            self.style_override = Some(bundle);
            self.theme.look = self.theme.look.with_bundle(bundle);
        }
        // El carril del transcript (`rail`): ausente deja `tree`; un valor desconocido avisa.
        self.load_rail_pref(&path);
        // Se aplica sin marcar `mouse_capture_dirty`: el arranque ya usa este valor.
        if let Some(on) = crate::prefs::load_mouse(&path) {
            self.mouse_capture = on;
        }
        // El muñeco de estado (`/mascot`) está encendido por defecto.
        if let Some(on) = crate::prefs::load_mascot(&path) {
            self.mascot = on;
        }
        // Los efectos del logo (`/logofx`): encendidos por defecto, con su intervalo.
        self.load_logo_fx_prefs(&path);
        // La lluvia parte de la config del tema y encima aplica el override
        // del usuario, si lo hay.
        let matrix = crate::prefs::load_matrix(&path, &self.theme.matrix);
        self.matrix_rain.set_config(matrix);
        // El spinner elegido en /temas › Spinner.
        if let Some(key) = crate::prefs::load_spinner(&path) {
            self.spinner_choice = SpinnerChoice::from_key(&key);
            self.composer
                .set_activity_style(self.spinner_choice.style_for(0));
        }
        self.prefs_path = Some(path);
    }

    /// Fija la captura del ratón, la marca como pendiente de aplicar y la persiste
    /// si hay `prefs_path`. Un fallo al guardar no impide el cambio: se avisa en
    /// `slash_feedback`.
    pub fn set_mouse_capture(&mut self, on: bool) {
        self.mouse_capture = on;
        self.mouse_capture_dirty = true;
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_mouse(path, on) {
                self.slash_feedback = Some(format!("Mouse setting not saved: {err}"));
            }
        }
    }

    /// Fija si el muñeco de estado se dibuja (`/mascot`), lo persiste si hay `prefs_path`
    /// y avisa en `slash_feedback` si el guardado falla, sin impedir el cambio.
    pub fn set_mascot(&mut self, on: bool) {
        self.mascot = on;
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_mascot(path, on) {
                self.slash_feedback = Some(format!("Mascot setting not saved: {err}"));
            }
        }
    }

    /// Invierte la captura del ratón (`/mouse`).
    pub fn toggle_mouse_capture(&mut self) {
        self.set_mouse_capture(!self.mouse_capture);
    }

    /// Fija si los avisos de proveedor van al transcript (`/verbose on|off`).
    /// El cambio es de sesión: no reescribe `loomcoding.toml`.
    pub fn set_provider_notices_verbose(&mut self, on: bool) {
        self.verbose_provider_notices = on;
    }

    /// Invierte los avisos de proveedor (`/verbose`) y devuelve el valor nuevo.
    pub fn toggle_provider_notices(&mut self) -> bool {
        self.verbose_provider_notices = !self.verbose_provider_notices;
        self.verbose_provider_notices
    }

    pub(super) fn apply_theme(&mut self, preset: loomcoding_cloth::widgets::ThemePreset) {
        self.theme_preset = preset;
        self.theme = loomcoding_cloth::widgets::Theme::preset(preset);
        // El estilo elegido por el usuario sustituye al del preset, como el layout.
        if let Some(bundle) = self.style_override {
            self.theme.look = self.theme.look.with_bundle(bundle);
        }
        // El preset trae su propio layout; el elegido por el usuario lo sustituye.
        if let Some(layout) = self.layout_override {
            self.theme.look.layout = layout;
        }
        // El tema nuevo nace sin la caja del usuario y la caché de alturas puede venir del anterior:
        // se vuelve a medir y se fija la caja según el carril elegido.
        self.layout.invalidate();
        self.fija_caja(self.rail_style == crate::render::rail::RailStyle::Box);
        self.sync_all_tools();
    }

    pub fn slash_query(&self) -> Option<&str> {
        let text = self.composer.text();
        if self.focus != Focus::Composer
            || self.composer.lines() != 1
            || self.permission.is_some()
            || self.session_picker_open
            || self.session_loading
            || self.settings_open
            || self.help_open
            || self.agents_open
            || self.a2a_open
            || self.tools_open
            || self.queue_open
            || self.slash_dismissed.as_deref() == Some(text)
        {
            return None;
        }
        let trimmed = text.trim_end();
        let query = trimmed.strip_prefix('/')?;
        let command = query.split_whitespace().next().unwrap_or(query);
        let has_args = command.len() < query.len();
        // `attach`, `verbose`, `logofx`, `model`, `effort`, `provider`, `ui`, `flow` y `help` aceptan argumentos y siguen mostrando el popup.
        if command.contains('/')
            || (has_args
                && !matches!(
                    command,
                    "attach"
                        | "verbose"
                        | "logofx"
                        | "model"
                        | "effort"
                        | "provider"
                        | "ui"
                        | "flow"
                        | "help"
                ))
        {
            return None;
        }
        Some(&trimmed[..command.len() + 1])
    }

    pub fn slash_matches(&self) -> Vec<(crate::commands::Command, &'static str, &'static str)> {
        let Some(query) = self.slash_query() else {
            return Vec::new();
        };
        crate::commands::COMMANDS
            .iter()
            .copied()
            .filter(|(command, name, _)| {
                !crate::commands_alias::is_hidden_alias(*command) && name.starts_with(query)
            })
            .collect()
    }
}
