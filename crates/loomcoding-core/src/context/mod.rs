//! Contexto estructural del repositorio.
pub mod cache;
pub mod index;
pub mod pack;
#[cfg(test)]
mod tests;
pub use index::{Reference, Symbol, SymbolIndex, SymbolKind};
