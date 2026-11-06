//! Refresco de los proveedores locales al abrir `/model`, `/settings` o el selector de proveedor.
//!
//! La interfaz solo avisa a través de un enlace (como `/refresh-agents`). El enfriamiento de cinco
//! segundos vive en quien sondea, así que el compositor nunca espera a un servidor local.

use super::*;

impl App {
    /// Enlaza el refresco de proveedores locales con quien sabe sondearlos.
    pub fn set_refresh_providers_hook(&mut self, hook: Arc<dyn Fn() + Send + Sync>) {
        self.refresh_providers_hook = Some(hook);
    }

    /// Pide un sondeo de los proveedores locales. Sin enlace no hace nada; nunca bloquea.
    pub(super) fn request_refresh_providers(&self) {
        if let Some(hook) = &self.refresh_providers_hook {
            hook();
        }
    }
}
