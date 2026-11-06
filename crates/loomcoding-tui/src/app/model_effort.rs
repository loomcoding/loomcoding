//! `/model` y `/effort` sin abrir `/settings`: fijan el modelo o el esfuerzo del proveedor actual
//! por el mismo camino de guardado (`settings_save_requested`) y, sin argumento, abren su selector
//! rápido. Cabe señalar que las reglas de turno en marcha y de ajustes ausentes son las de
//! `/settings`, para que ambos caminos no puedan divergir.

use super::*;

impl App {
    /// Fija el modelo del proveedor actual a `id` y pide guardarlo. Acepta cualquier identificador
    /// sin espacios, también uno que el catálogo no conozca. Con el mismo identificador no reenvía nada.
    pub fn set_model_now(&mut self, id: &str) -> Result<(), String> {
        let id = id.trim();
        if id.is_empty() || id.chars().any(char::is_whitespace) {
            return Err("Usage: /model <id>".into());
        }
        let Some(current) = self.provider_settings.as_ref() else {
            return Err("Provider settings are unavailable".into());
        };
        // Misma regla que `/settings`: con un turno en marcha no se aplica nada.
        if self.turn_in_flight {
            return Err("Wait for the active turn before applying settings".into());
        }
        if self.settings_saving {
            return Err("Settings are still being saved".into());
        }
        if current.model == id {
            self.slash_feedback = Some(format!("model: {id} (unchanged)"));
            return Ok(());
        }
        if let Some(settings) = self.provider_settings.as_mut() {
            settings.model = id.to_string();
        }
        // `reconcile_effort` deja su aviso en `settings_feedback` solo si descarta el esfuerzo.
        self.settings_feedback = None;
        self.reconcile_effort();
        let effort_reset = self.settings_feedback.take().is_some();
        self.settings_save_requested = true;
        self.slash_feedback = Some(if effort_reset {
            format!("model: {id} · effort reset")
        } else {
            format!("model: {id}")
        });
        Ok(())
    }

    /// `/model` sin argumento: abre el selector de modelos del proveedor actual en modo rápido.
    /// Sin ajustes, o sin catálogo para ese proveedor, no abre nada y lo dice.
    pub fn open_model_picker_now(&mut self) -> Result<(), String> {
        if self.provider_settings.is_none() {
            return Err("Provider settings are unavailable".into());
        }
        self.request_refresh_providers();
        let has_models = self
            .model_catalog_provider()
            .is_some_and(|provider| !provider.models.is_empty());
        if !has_models {
            return Err("No model catalog for this provider: use /model <id>".into());
        }
        self.open_picker(SettingsPicker::Model);
        self.quick_model_picker = self.settings_picker == Some(SettingsPicker::Model);
        Ok(())
    }

    /// Cierra el selector rápido sin tocar el modelo ni los ajustes.
    pub fn close_quick_model_picker(&mut self) {
        self.quick_model_picker = false;
        self.close_picker();
    }

    /// `Enter` en el selector rápido: aplica la fila resaltada con `set_model_now`. Si el filtro no
    /// coincide con ningún modelo, aplica el texto escrito, como hace `/settings`. Cierra el
    /// selector en cualquier caso; un rechazo queda en `slash_feedback`.
    pub fn choose_quick_model(&mut self) {
        let items = self.picker_items();
        let id = items.get(self.picker_selected).map_or_else(
            || self.picker_query.trim().to_string(),
            |item| item.id.clone(),
        );
        self.close_quick_model_picker();
        if let Err(message) = self.set_model_now(&id) {
            self.slash_feedback = Some(message);
        }
    }

    /// Fija el esfuerzo de razonamiento del modelo actual. `None` o «default» (sin distinguir
    /// mayúsculas ni espacios) quita el nivel. Los niveles se comparan sin distinguir mayúsculas,
    /// pero se guarda la forma exacta de `effort_options()`. Con el mismo nivel no reenvía nada.
    pub fn set_effort_now(&mut self, level: Option<&str>) -> Result<(), String> {
        let Some(current) = self.provider_settings.as_ref() else {
            return Err("Provider settings are unavailable".into());
        };
        // Misma regla que `/settings`: con un turno en marcha no se aplica nada.
        if self.turn_in_flight {
            return Err("Wait for the active turn before applying settings".into());
        }
        if self.settings_saving {
            return Err("Settings are still being saved".into());
        }
        let options = self.effort_options();
        if options.is_empty() {
            return Err(NO_EFFORT_LEVELS.into());
        }
        let wanted = level.map(str::trim).unwrap_or("default");
        let next = if wanted.eq_ignore_ascii_case("default") {
            None
        } else {
            match options
                .iter()
                .find(|option| option.eq_ignore_ascii_case(wanted))
            {
                Some(option) => Some(option.clone()),
                None => return Err(format!("Usage: /effort [default|{}]", options.join("|"))),
            }
        };
        let label = next.clone().unwrap_or_else(|| "default".into());
        if current.effort == next {
            self.slash_feedback = Some(format!("effort: {label} (unchanged)"));
            return Ok(());
        }
        if let Some(settings) = self.provider_settings.as_mut() {
            settings.effort = next;
        }
        self.settings_save_requested = true;
        self.slash_feedback = Some(format!("effort: {label}"));
        Ok(())
    }

    /// `/effort` sin argumento: abre el selector de esfuerzo con «default» y los niveles, con la
    /// fila del vigente resaltada. Sin ajustes, o sin niveles para el modelo, no abre nada.
    pub fn open_effort_picker_now(&mut self) -> Result<(), String> {
        let Some(current) = self.provider_settings.as_ref() else {
            return Err("Provider settings are unavailable".into());
        };
        let options = self.effort_options();
        if options.is_empty() {
            return Err(NO_EFFORT_LEVELS.into());
        }
        let selected = current
            .effort
            .as_ref()
            .and_then(|level| options.iter().position(|option| option == level))
            .map_or(0, |index| index + 1);
        self.quick_effort_picker = Some(selected);
        Ok(())
    }

    /// Cierra el selector de esfuerzo sin tocar el nivel ni los ajustes.
    pub fn close_quick_effort_picker(&mut self) {
        self.quick_effort_picker = None;
    }

    /// Mueve la fila resaltada del selector de esfuerzo, sin dar la vuelta al llegar a un extremo.
    pub fn effort_picker_move(&mut self, down: bool) {
        let Some(selected) = self.quick_effort_picker else {
            return;
        };
        let last = self.effort_options().len();
        self.quick_effort_picker = Some(if down {
            (selected + 1).min(last)
        } else {
            selected.saturating_sub(1)
        });
    }

    /// `Enter` en el selector de esfuerzo: aplica la fila resaltada con `set_effort_now`. Cierra
    /// el selector en cualquier caso; un rechazo queda en `slash_feedback`.
    pub fn choose_quick_effort(&mut self) {
        let selected = self.quick_effort_picker.unwrap_or(0);
        let level = selected
            .checked_sub(1)
            .and_then(|index| self.effort_options().get(index).cloned());
        self.close_quick_effort_picker();
        if let Err(message) = self.set_effort_now(level.as_deref()) {
            self.slash_feedback = Some(message);
        }
    }
}

/// Aviso para un modelo sin niveles de razonamiento ni `[provider] efforts` declarados.
const NO_EFFORT_LEVELS: &str =
    "This model has no reasoning levels. Declare them in [provider] efforts of loomcoding.toml";
