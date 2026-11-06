//! `/provider` sin abrir `/settings`: fija el proveedor (tipo, URL, variable de clave y modelo) por el
//! mismo camino de guardado que `/model`. Sin argumento abre un selector rápido con los proveedores
//! del catálogo, con su estado (`up`, `down`, `sin acceso`) en el nombre.
//!
//! Cabe señalar que solo se aceptan los proveedores del catálogo, los locales y los de models.dev,
//! para que el argumento y el selector vean exactamente la misma lista.

use super::*;

/// Cuántos identificadores válidos se listan en el aviso de un id desconocido; el resto se resume.
const MAX_IDS_EN_AVISO: usize = 12;

impl App {
    /// Fija el proveedor `id` del catálogo y pide guardarlo. Como modelo toma el actual si el
    /// proveedor lo publica, o si no el primero de sus modelos. Con el mismo proveedor no reenvía nada.
    pub fn set_provider_now(&mut self, id: &str) -> Result<(), String> {
        let id = id.trim();
        if id.is_empty() || id.chars().any(char::is_whitespace) {
            return Err("Usage: /provider [id]".into());
        }
        let Some(current) = self.provider_settings.as_ref() else {
            return Err("Provider settings are unavailable".into());
        };
        // Misma regla que `/settings` y `/model`: con un turno en marcha no se aplica nada.
        if self.turn_in_flight {
            return Err("Wait for the active turn before applying settings".into());
        }
        if self.settings_saving {
            return Err("Settings are still being saved".into());
        }
        if self.catalog.is_empty() {
            return Err("No provider catalog yet: use /settings".into());
        }
        let catalog = Arc::clone(&self.catalog);
        let Some(provider) = catalog.provider(id) else {
            return Err(self.provider_desconocido(id));
        };
        let Some(first) = provider.models.first() else {
            return Err(format!(
                "{} has no models in the catalog: use /settings",
                provider.id
            ));
        };
        let model = if provider.models.iter().any(|m| m.id == current.model) {
            current.model.clone()
        } else {
            first.id.clone()
        };
        let same = current.kind == provider.kind
            && current.base_url == provider.base_url
            && current.api_key_env == provider.api_key_env
            && current.model == model;
        if same {
            self.slash_feedback = Some(format!("provider: {id} (unchanged)"));
            return Ok(());
        }
        if let Some(settings) = self.provider_settings.as_mut() {
            settings.kind = provider.kind;
            settings.base_url = provider.base_url.clone();
            settings.api_key_env = provider.api_key_env.clone();
            settings.model = model.clone();
        }
        self.settings_api_key.clear();
        // Igual que `/model`: el esfuerzo se reconcilia con los niveles del proveedor nuevo.
        self.settings_feedback = None;
        self.reconcile_effort();
        let effort_reset = self.settings_feedback.take().is_some();
        self.settings_save_requested = true;
        self.slash_feedback = Some(if effort_reset {
            format!("provider: {id} · model: {model} · effort reset")
        } else {
            format!("provider: {id} · model: {model}")
        });
        Ok(())
    }

    /// Aviso de un id que el catálogo no conoce, con la lista de ids válidos (recortada).
    fn provider_desconocido(&self, id: &str) -> String {
        let ids: Vec<&str> = self
            .catalog
            .providers
            .iter()
            .map(|provider| provider.id.as_str())
            .collect();
        let mut lista = ids
            .iter()
            .take(MAX_IDS_EN_AVISO)
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        if ids.len() > MAX_IDS_EN_AVISO {
            lista.push_str(&format!(" … (+{})", ids.len() - MAX_IDS_EN_AVISO));
        }
        format!("Unknown provider: {id} · valid: {lista}")
    }

    /// `/provider` sin argumento: abre el selector de proveedores y pide un sondeo de los locales.
    /// Sin ajustes, o sin catálogo, no abre nada y lo dice.
    pub fn open_provider_picker_now(&mut self) -> Result<(), String> {
        if self.provider_settings.is_none() {
            return Err("Provider settings are unavailable".into());
        }
        self.request_refresh_providers();
        if self.catalog.is_empty() {
            return Err("No provider catalog yet: use /settings".into());
        }
        self.quick_provider_picker = true;
        self.picker_query.clear();
        self.picker_selected = 0;
        Ok(())
    }

    /// Filas del selector de proveedores: los del catálogo que casan con el filtro (id o nombre).
    /// El nombre lleva el estado del sondeo, p. ej. «llama-server · up (3)».
    pub fn provider_items(&self) -> Vec<PickerItem> {
        let query = self.picker_query.trim().to_lowercase();
        self.catalog
            .providers
            .iter()
            .filter(|provider| {
                query.is_empty()
                    || provider.id.to_lowercase().contains(&query)
                    || provider.name.to_lowercase().contains(&query)
            })
            .map(|provider| PickerItem {
                id: provider.id.clone(),
                label: provider.name.clone(),
                detail: format!("{} · {}", provider.id, provider.base_url),
            })
            .collect()
    }

    /// Id del catálogo del proveedor vigente, si el catálogo lo tiene (se busca por URL base).
    pub fn current_provider_id(&self) -> Option<String> {
        self.model_catalog_provider()
            .map(|provider| provider.id.clone())
    }

    /// Cierra el selector de proveedores sin tocar los ajustes.
    pub fn close_quick_provider_picker(&mut self) {
        self.quick_provider_picker = false;
        self.picker_query.clear();
        self.picker_selected = 0;
    }

    /// Mueve la fila resaltada del selector de proveedores, sin dar la vuelta al llegar al final.
    pub fn provider_picker_move(&mut self, down: bool) {
        let last = self.provider_items().len().saturating_sub(1);
        self.picker_selected = if down {
            (self.picker_selected + 1).min(last)
        } else {
            self.picker_selected.saturating_sub(1)
        };
    }

    /// `Enter` en el selector de proveedores: aplica la fila resaltada con `set_provider_now`. Con
    /// el filtro sin coincidencias, prueba el texto escrito como id. Cierra el selector siempre; un
    /// rechazo queda en `slash_feedback`.
    pub fn choose_quick_provider(&mut self) {
        let items = self.provider_items();
        let id = items.get(self.picker_selected).map_or_else(
            || self.picker_query.trim().to_string(),
            |item| item.id.clone(),
        );
        self.close_quick_provider_picker();
        if let Err(message) = self.set_provider_now(&id) {
            self.slash_feedback = Some(message);
        }
    }
}
