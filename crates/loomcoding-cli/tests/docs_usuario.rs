//! Guardia de la documentación de usuario (`docs/user/{en,es}/**/*.md`).
//!
//! La comprobación vive en `docs_usuario/guardia.rs`; las pruebas unitarias y la que recorre
//! el árbol real, en `docs_usuario/pruebas.rs`.

#[path = "docs_usuario/guardia.rs"]
mod guardia;

#[cfg(test)]
#[path = "docs_usuario/pruebas.rs"]
mod pruebas;
