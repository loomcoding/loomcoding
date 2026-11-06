//! `/model` sin abrir `/settings`: fija el modelo del proveedor actual por el mismo camino de
//! guardado (`settings_save_requested`) y, sin argumento, abre el selector de modelos en modo
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
}
