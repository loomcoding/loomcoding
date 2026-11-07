//! Contribuciones al prompt de sistema del agente: punto de extensión genérico que `system_prompt`
//! consulta tras la guía de delegación y antes del prompt propio del agente.
//!
//! Cabe señalar que el núcleo no sabe qué aporta quien lo implementa: solo lo consulta, lo acota y
//! lo une. Las reglas que cumple el núcleo, sea cual sea el contribuidor:
//! - un contribuidor que entra en pánico se omite; el resto del prompt sale igual;
//! - cada sección se acota a [`MAX_CONTRIBUTED_SECTION_BYTES`];
//! - una sección que devuelve `None` o solo blancos no añade separadores.
//!
//! El contribuidor debe ser barato: se consulta en cada turno, y su texto debe ser estable entre
//! turnos para no romper la caché del prompt.
//!
//! Solo lo tiene el agente al que se le configura con `with_prompt_contributor`: los subagentes
//! locales se construyen sin contribuidores.

use super::*;
use std::panic::AssertUnwindSafe;

/// Bytes máximos de cada sección que un contribuidor aporta al prompt de sistema.
pub const MAX_CONTRIBUTED_SECTION_BYTES: usize = 2_000;

/// Aporta una sección al prompt de sistema del agente. `None` no aporta nada.
pub trait PromptContributor: Send + Sync {
    fn section(&self) -> Option<String>;
}

impl AgentLoop {
    /// Añade un contribuidor de prompt. Se consultan en el orden en que se añadieron.
    pub fn with_prompt_contributor(mut self, contributor: Arc<dyn PromptContributor>) -> Self {
        self.prompt_contributors.push(contributor);
        self
    }

    /// Secciones de todos los contribuidores, acotadas y unidas por una línea en blanco. `None` si
    /// ninguno aporta texto.
    pub(super) fn contributed_sections(&self) -> Option<String> {
        let secciones: Vec<String> = self
            .prompt_contributors
            .iter()
            .filter_map(|contributor| {
                // Un pánico del contribuidor descarta solo su sección.
                std::panic::catch_unwind(AssertUnwindSafe(|| contributor.section()))
                    .ok()
                    .flatten()
            })
            .filter(|section| !section.trim().is_empty())
            .map(|section| limit_bytes(&section, MAX_CONTRIBUTED_SECTION_BYTES).into_owned())
            .collect();
        (!secciones.is_empty()).then(|| secciones.join("\n\n"))
    }
}
