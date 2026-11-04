//! Secreto que nunca se muestra, ni en `Debug` ni en `Display`.

use super::*;

/// Un valor secreto. No se puede mostrar, copiar ni serializar por descuido.
pub struct Secret(pub(super) String);

impl Secret {
    /// `None` si el valor está vacío: una variable vacía equivale a una variable ausente.
    pub fn new(valor: String) -> Option<Secret> {
        (!valor.trim().is_empty()).then_some(Secret(valor))
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("«secreto»")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("«secreto»")
    }
}
