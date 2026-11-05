use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use tree_sitter::{Parser, Query};

const TAGS_SCM: &str = include_str!("tags.scm");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Type,
    Trait,
    Module,
    Macro,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub path: PathBuf,
    pub line: u32,
    pub signature: String,
}

/// Lo que se extrae de un fichero: definiciones y llamadas (nombre y línea).
type FileFacts = (Vec<Symbol>, Vec<(String, u32)>);

/// Uso (llamada) de un símbolo: ruta relativa y línea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub path: PathBuf,
    pub line: u32,
}

pub struct SymbolIndex {
    defs: HashMap<String, Vec<Symbol>>,
    refs: HashMap<String, Vec<Reference>>,
}

impl SymbolIndex {
    pub fn build(root: &Path) -> SymbolIndex {
        let mut index = SymbolIndex {
            defs: HashMap::new(),
            refs: HashMap::new(),
        };

        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let query = Query::new(&lang, TAGS_SCM).unwrap();
        let mut parser = Parser::new();
        parser.set_language(&lang).unwrap();

        let walker = WalkBuilder::new(root)
            .require_git(false)
            .filter_entry(|e| {
                let n = e.file_name().to_string_lossy();
                n != "target" && n != ".git"
            })
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }

            let relative_path = path.strip_prefix(root).unwrap_or(path);
            let src = match fs::read(path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some((symbols, calls)) =
                Self::analyze_file(&src, relative_path, &query, &mut parser)
            {
                for sym in symbols {
                    index.defs.entry(sym.name.clone()).or_default().push(sym);
                }
                for (name, line) in calls {
                    index.refs.entry(name).or_default().push(Reference {
                        path: relative_path.to_path_buf(),
                        line,
                    });
                }
            }
        }

        index
    }

    /// Definiciones y llamadas (nombre y línea) de un fichero.
    fn analyze_file(
        src: &[u8],
        path: &Path,
        query: &Query,
        parser: &mut Parser,
    ) -> Option<FileFacts> {
        use streaming_iterator::StreamingIterator;
        use tree_sitter::QueryCursor;

        let tree = parser.parse(src, None)?;

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), src);

        let mut symbols = Vec::new();
        let mut calls: Vec<(String, u32)> = Vec::new();

        while let Some(m) = matches.next() {
            let mut name: Option<String> = None;
            let mut name_line = 0u32;
            let mut call = false;
            let mut def: Option<(&str, tree_sitter::Node)> = None;

            for c in m.captures {
                let cap = query.capture_names()[c.index as usize];
                if cap == "name" {
                    name = c.node.utf8_text(src).ok().map(str::to_string);
                    name_line = c.node.start_position().row as u32 + 1;
                } else if cap == "reference.call" {
                    call = true;
                } else if cap.starts_with("definition.") {
                    def = Some((cap, c.node));
                }
            }

            if call {
                if let Some(name) = name.filter(|n| !n.is_empty()) {
                    calls.push((name, name_line));
                }
                continue;
            }
            let (Some(name), Some((cap, node))) = (name, def) else {
                continue;
            };

            if name.is_empty() {
                continue;
            }

            let definition_kind = match cap {
                "definition.class" => SymbolKind::Type,
                "definition.method" => SymbolKind::Method,
                "definition.function" => SymbolKind::Function,
                "definition.interface" => SymbolKind::Trait,
                "definition.module" => SymbolKind::Module,
                "definition.macro" => SymbolKind::Macro,
                _ => continue,
            };

            let line = node.start_position().row as u32 + 1;

            let definition_text = match node.utf8_text(src) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let first_newline = definition_text.find('\n');
            let signature = if let Some(pos) = first_newline {
                definition_text[..pos].to_string()
            } else {
                definition_text.to_string()
            };

            let trimmed_signature = signature.trim().to_string();

            symbols.push(Symbol {
                name,
                kind: definition_kind,
                path: path.to_path_buf(),
                line,
                signature: trimmed_signature,
            });
        }

        // `tags.scm` registra cada método dos veces (como método y como función): se queda el método.
        let methods: std::collections::HashSet<(String, u32)> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .map(|s| (s.name.clone(), s.line))
            .collect();
        symbols.retain(|s| {
            !(s.kind == SymbolKind::Function && methods.contains(&(s.name.clone(), s.line)))
        });

        if symbols.is_empty() && calls.is_empty() {
            None
        } else {
            Some((symbols, calls))
        }
    }

    /// Llamadas conocidas a `name`, en el orden en que se recorrió el repositorio.
    pub fn references(&self, name: &str) -> &[Reference] {
        self.refs.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Nombres con al menos una definición, para buscar por aproximación.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.defs.keys().map(String::as_str)
    }

    pub fn definitions(&self, name: &str) -> &[Symbol] {
        self.defs.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Rutas (relativas a la raíz) con al menos una definición, ordenadas y sin repetir.
    pub fn files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = self
            .defs
            .values()
            .flatten()
            .map(|s| s.path.clone())
            .collect();
        files.sort();
        files.dedup();
        files
    }
}
