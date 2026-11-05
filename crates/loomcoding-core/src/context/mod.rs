//! Contexto estructural del repositorio.
pub mod index;
#[cfg(test)]
mod tests;
pub use index::{Symbol, SymbolIndex, SymbolKind};
