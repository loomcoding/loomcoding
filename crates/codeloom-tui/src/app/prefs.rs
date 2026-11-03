//! Tema, ratón, avisos del proveedor y filtro de comandos con barra.

use super::*;

impl App {
    /// Selecciona el look inicial de la TUI de CodeLoom.
    pub fn use_product_default_view(&mut self) {
        self.set_theme(batten::widgets::ThemePreset::Crush);
    }

    /// Pasa al siguiente preset de la rotación (`/theme`).
    pub fn cycle_theme(&mut self) {
        use batten::widgets::ThemePreset;
        let presets = ThemePreset::ALL;
        let current = presets
            .iter()
            .position(|preset| *preset == self.theme_preset)
            .unwrap_or(0);
        self.set_theme(presets[(current + 1) % presets.len()]);
    }

    /// Fija el tema, lo persiste si hay `prefs_path` y reaplica el look a lo ya dibujado.
    /// Un fallo al guardar no impide el cambio: se avisa en `slash_feedback`.
    pub fn set_theme(&mut self, preset: batten::widgets::ThemePreset) {
        self.apply_theme(preset);
        self.slash_feedback = None;
        if let Some(path) = &self.prefs_path {
            if let Err(err) = crate::prefs::save_theme(path, preset) {
                self.slash_feedback = Some(format!("Theme not saved: {err}"));
            }
        }
    }

    /// Fija el fichero de preferencias y aplica el tema guardado en él, si lo hay.
    /// No vuelve a escribir el fichero al aplicarlo.
    pub fn set_prefs_path(&mut self, path: std::path::PathBuf) {
        if let Some(preset) = crate::prefs::load_theme(&path) {
            self.apply_theme(preset);
        }
        // Se aplica sin marcar `mouse_capture_dirty`: el arranque ya usa este valor.
        if let Some(on) = crate::prefs::load_mouse(&path) {
            self.mouse_capture = on;
        }
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

    /// Invierte la captura del ratón (`/mouse`).
    pub fn toggle_mouse_capture(&mut self) {
        self.set_mouse_capture(!self.mouse_capture);
    }

    /// Fija si los avisos de proveedor van al transcript (`/verbose on|off`).
    /// El cambio es de sesión: no reescribe `codeloom.toml`.
    pub fn set_provider_notices_verbose(&mut self, on: bool) {
        self.verbose_provider_notices = on;
    }

    /// Invierte los avisos de proveedor (`/verbose`) y devuelve el valor nuevo.
    pub fn toggle_provider_notices(&mut self) -> bool {
        self.verbose_provider_notices = !self.verbose_provider_notices;
        self.verbose_provider_notices
    }

    pub(super) fn apply_theme(&mut self, preset: batten::widgets::ThemePreset) {
        self.theme_preset = preset;
        self.theme = batten::widgets::Theme::preset(preset);
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
        // `attach` y `verbose` aceptan argumentos y siguen mostrando el popup.
        if command.contains('/') || (has_args && command != "attach" && command != "verbose") {
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
            .filter(|(_, name, _)| name.starts_with(query))
            .collect()
    }
}
