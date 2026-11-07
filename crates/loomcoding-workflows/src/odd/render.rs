//! Ida y vuelta entre [`FeatureDoc`] y Markdown.
//!
//! Formato: `# <feature>`, luego las secciones `## Objetivo`, `## Motivo`, `## Alcance`,
//! `## Restricciones`, `## Tareas`, `## Registro` y `## Siguiente paso`. Las tareas son
//! `- [ ] T1 texto`, `- [x] T1 texto — evidencia: …` o `- [!] T1 texto — bloqueada: motivo`.

use super::doc::{
    limit_bytes, FeatureDoc, Task, TaskStatus, MAX_DOC_BYTES, MAX_EVIDENCE_BYTES, MAX_LOG_LINES,
    MAX_LOG_LINE_BYTES, MAX_TASKS, MAX_TASK_TEXT_BYTES,
};
use super::OddError;

/// Separadores del formato. Son `pub(crate)` para que `odd_track` rechace los textos del modelo que
/// los contengan: si los contuvieran, el documento no haría ida y vuelta exacta.
pub(crate) const EVIDENCE_MARK: &str = " — evidencia:";
pub(crate) const BLOCK_MARK: &str = " — bloqueada:";

impl FeatureDoc {
    /// Serializa el documento. Los valores se recortan y los saltos de línea de las
    /// tareas y del registro se aplanan, para que cada elemento ocupe una línea.
    pub fn to_markdown(&self) -> String {
        let mut out = format!("# {}\n\n", aplanar(self.feature.trim()));
        seccion(&mut out, "Objetivo", &self.objective);
        seccion(&mut out, "Motivo", &self.why);
        seccion(&mut out, "Alcance", &self.scope);
        seccion(&mut out, "Restricciones", &self.constraints);
        out.push_str("## Tareas\n\n");
        for t in &self.tasks {
            out.push_str(&linea_de_tarea(t));
            out.push('\n');
        }
        out.push_str("\n## Registro\n\n");
        for entrada in &self.log {
            let limpia = aplanar(entrada.trim());
            if !limpia.is_empty() {
                out.push_str("- ");
                out.push_str(&limpia);
                out.push('\n');
            }
        }
        out.push('\n');
        seccion(&mut out, "Siguiente paso", &self.next_step);
        out
    }

    /// Lee un documento. Rechaza lo que supera las cotas, lo que no tiene cabecera y las tareas
    /// mal formadas o repetidas. Las casillas `[x]` sin evidencia se aceptan tal cual.
    pub fn parse(input: &str) -> Result<FeatureDoc, OddError> {
        if input.len() > MAX_DOC_BYTES {
            return Err(OddError::TooLarge {
                size: input.len(),
                max: MAX_DOC_BYTES,
            });
        }
        let mut doc = FeatureDoc::default();
        let mut cabecera = false;
        let mut actual = Seccion::Otra;
        let mut cuerpo: Vec<&str> = Vec::new();

        for (i, raw) in input.lines().enumerate() {
            let n = i + 1;
            if !cabecera {
                if raw.trim().is_empty() {
                    continue;
                }
                doc.feature = parse_cabecera(n, raw)?;
                cabecera = true;
                continue;
            }
            if let Some(titulo) = raw.strip_prefix("## ") {
                volcar(&mut doc, actual, &cuerpo);
                cuerpo.clear();
                actual = Seccion::de_titulo(titulo.trim());
                continue;
            }
            match actual {
                Seccion::Tareas => {
                    if let Some(item) = raw.trim().strip_prefix("- ") {
                        let tarea = parse_tarea(n, item)?;
                        if doc.tasks.len() >= MAX_TASKS {
                            return Err(OddError::TooManyTasks {
                                count: doc.tasks.len() + 1,
                                max: MAX_TASKS,
                            });
                        }
                        if doc.tasks.iter().any(|t| t.id == tarea.id) {
                            return Err(OddError::DuplicateTask { id: tarea.id });
                        }
                        doc.tasks.push(tarea);
                    }
                }
                Seccion::Registro => {
                    let t = raw.trim();
                    let entrada = t.strip_prefix("- ").unwrap_or(t).trim();
                    if !entrada.is_empty() {
                        let acotada = limit_bytes(entrada, MAX_LOG_LINE_BYTES).trim_end();
                        doc.log.push(acotada.to_string());
                    }
                }
                Seccion::Otra => {}
                _ => cuerpo.push(raw),
            }
        }
        if !cabecera {
            return Err(OddError::MissingHeader);
        }
        volcar(&mut doc, actual, &cuerpo);
        if doc.log.len() > MAX_LOG_LINES {
            let sobran = doc.log.len() - MAX_LOG_LINES;
            doc.log.drain(..sobran);
        }
        Ok(doc)
    }
}

/// Sustituye saltos de línea por espacios, para que un valor ocupe una sola línea.
fn aplanar(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect()
}

/// Escribe una sección de texto libre; el cuerpo va recortado.
fn seccion(out: &mut String, titulo: &str, cuerpo: &str) {
    out.push_str("## ");
    out.push_str(titulo);
    out.push_str("\n\n");
    let cuerpo = cuerpo.trim();
    if !cuerpo.is_empty() {
        out.push_str(cuerpo);
        out.push('\n');
    }
    out.push('\n');
}

/// Cola de una tarea: ` — evidencia: …` o ` — bloqueada: …`, si hay valor no vacío.
fn cola(marca: &str, valor: Option<&str>) -> String {
    match valor.map(|v| aplanar(v.trim())).filter(|v| !v.is_empty()) {
        Some(v) => format!("{marca} {v}"),
        None => String::new(),
    }
}

fn linea_de_tarea(t: &Task) -> String {
    let texto = aplanar(t.text.trim());
    let (casilla, sufijo) = match t.status {
        TaskStatus::Pending => (' ', String::new()),
        TaskStatus::Done => ('x', cola(EVIDENCE_MARK, t.evidence.as_deref())),
        TaskStatus::Blocked => ('!', cola(BLOCK_MARK, t.reason.as_deref())),
    };
    format!("- [{casilla}] {} {texto}{sufijo}", t.id)
}

/// Reparte el cuerpo de una tarea en texto y valor, por la primera aparición de la marca.
fn partir<'a>(cuerpo: &'a str, marca: &str) -> (&'a str, Option<&'a str>) {
    match cuerpo.find(marca) {
        Some(i) => (cuerpo[..i].trim(), Some(cuerpo[i + marca.len()..].trim())),
        None => (cuerpo, None),
    }
}

fn parse_cabecera(n: usize, raw: &str) -> Result<String, OddError> {
    let Some(titulo) = raw.strip_prefix("# ") else {
        return Err(OddError::MissingHeader);
    };
    let feature = titulo.trim();
    if feature.is_empty() {
        return Err(OddError::Malformed {
            line: n,
            reason: "la cabecera no tiene nombre de feature",
        });
    }
    Ok(feature.to_string())
}

/// Lee una tarea a partir de lo que sigue a `- `: `[x] T1 texto — evidencia: …`.
fn parse_tarea(n: usize, item: &str) -> Result<Task, OddError> {
    let mal = |reason: &'static str| OddError::Malformed { line: n, reason };
    let mut cs = item.chars();
    if cs.next() != Some('[') {
        return Err(mal(
            "la tarea debe empezar por una casilla `[ ]`, `[x]` o `[!]`",
        ));
    }
    let status = match cs.next() {
        Some(' ') => TaskStatus::Pending,
        Some('x' | 'X') => TaskStatus::Done,
        Some('!') => TaskStatus::Blocked,
        _ => return Err(mal("casilla no reconocida; use `[ ]`, `[x]` o `[!]`")),
    };
    if cs.next() != Some(']') {
        return Err(mal("falta el `]` de la casilla"));
    }
    let resto = cs.as_str().trim();
    let (id, cuerpo) = resto.split_once(char::is_whitespace).unwrap_or((resto, ""));
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(mal("identificador de tarea no válido"));
    }
    let (texto, valor) = match status {
        TaskStatus::Pending => (cuerpo.trim(), None),
        TaskStatus::Done => partir(cuerpo.trim(), EVIDENCE_MARK),
        TaskStatus::Blocked => partir(cuerpo.trim(), BLOCK_MARK),
    };
    if texto.is_empty() {
        return Err(mal("la tarea no tiene texto"));
    }
    let too_long = |campo: &'static str, size: usize, max: usize| OddError::TaskFieldTooLong {
        id: id.to_string(),
        field: campo,
        size,
        max,
    };
    if texto.len() > MAX_TASK_TEXT_BYTES {
        return Err(too_long("texto", texto.len(), MAX_TASK_TEXT_BYTES));
    }
    let valor = valor.filter(|v| !v.is_empty());
    let (evidence, reason) = match (status, valor) {
        (TaskStatus::Done, Some(v)) => {
            if v.len() > MAX_EVIDENCE_BYTES {
                return Err(too_long("evidencia", v.len(), MAX_EVIDENCE_BYTES));
            }
            (Some(v.to_string()), None)
        }
        (TaskStatus::Blocked, Some(v)) => {
            if v.len() > MAX_TASK_TEXT_BYTES {
                return Err(too_long("motivo", v.len(), MAX_TASK_TEXT_BYTES));
            }
            (None, Some(v.to_string()))
        }
        _ => (None, None),
    };
    Ok(Task {
        id: id.to_string(),
        text: texto.to_string(),
        status,
        evidence,
        reason,
    })
}

/// Secciones reconocidas del documento. Las desconocidas se ignoran.
#[derive(Clone, Copy)]
enum Seccion {
    Objetivo,
    Motivo,
    Alcance,
    Restricciones,
    Tareas,
    Registro,
    Siguiente,
    Otra,
}

impl Seccion {
    fn de_titulo(titulo: &str) -> Self {
        match titulo {
            "Objetivo" => Self::Objetivo,
            "Motivo" => Self::Motivo,
            "Alcance" => Self::Alcance,
            "Restricciones" => Self::Restricciones,
            "Tareas" => Self::Tareas,
            "Registro" => Self::Registro,
            "Siguiente paso" => Self::Siguiente,
            _ => Self::Otra,
        }
    }
}

/// Guarda el cuerpo acumulado de una sección de texto libre. Las de tareas y registro ya se
/// leyeron línea a línea.
fn volcar(doc: &mut FeatureDoc, seccion: Seccion, cuerpo: &[&str]) {
    let texto = cuerpo.join("\n");
    let texto = texto.trim().to_string();
    match seccion {
        Seccion::Objetivo => doc.objective = texto,
        Seccion::Motivo => doc.why = texto,
        Seccion::Alcance => doc.scope = texto,
        Seccion::Restricciones => doc.constraints = texto,
        Seccion::Siguiente => doc.next_step = texto,
        Seccion::Tareas | Seccion::Registro | Seccion::Otra => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ejemplo() -> FeatureDoc {
        FeatureDoc {
            feature: "exportación de informes".into(),
            objective: "Exportar a PDF sin pérdida, con 😀 y 漢字.".into(),
            why: "Los clientes piden «informes» completos.".into(),
            scope: "Solo el módulo de informes; sin red.".into(),
            constraints: "Sin ratatui en el núcleo; ficheros ≤ 400 líneas.".into(),
            tasks: vec![
                Task {
                    id: "T1".into(),
                    text: "Modelo de datos — ñandú".into(),
                    status: TaskStatus::Done,
                    evidence: Some("cargo test: 12 passed".into()),
                    reason: None,
                },
                Task {
                    id: "T2".into(),
                    text: "Render en 日本語 😀".into(),
                    status: TaskStatus::Pending,
                    evidence: None,
                    reason: None,
                },
                Task {
                    id: "T3".into(),
                    text: "Bloqueada por API externa".into(),
                    status: TaskStatus::Blocked,
                    evidence: None,
                    reason: Some("falta la clave de la API".into()),
                },
            ],
            log: vec![
                "Documento creado.".into(),
                "T1 marcada con evidencia.".into(),
            ],
            next_step: "Empezar por T2.".into(),
        }
    }

    #[test]
    fn ida_y_vuelta_sin_perdida() {
        let doc = ejemplo();
        let md = doc.to_markdown();
        assert_eq!(FeatureDoc::parse(&md), Ok(doc));
    }

    #[test]
    fn formato_contiene_cabecera_secciones_y_formas_de_tarea() {
        let md = ejemplo().to_markdown();
        assert!(md.starts_with("# exportación de informes\n"));
        for titulo in [
            "## Objetivo",
            "## Motivo",
            "## Alcance",
            "## Restricciones",
            "## Tareas",
            "## Registro",
            "## Siguiente paso",
        ] {
            assert!(md.contains(titulo), "falta {titulo}");
        }
        assert!(md.contains("- [x] T1 Modelo de datos — ñandú — evidencia: cargo test: 12 passed"));
        assert!(md.contains("- [ ] T2 Render en 日本語 😀"));
        assert!(
            md.contains("- [!] T3 Bloqueada por API externa — bloqueada: falta la clave de la API")
        );
    }

    #[test]
    fn casilla_marcada_a_mano_sin_evidencia_se_respeta() {
        let md = "# f\n\n## Tareas\n\n- [x] T1 hecha a mano\n";
        let doc = FeatureDoc::parse(md).expect("debe aceptarse");
        assert_eq!(doc.tasks[0].status, TaskStatus::Done);
        assert_eq!(doc.tasks[0].evidence, None);
        assert_eq!(doc.tasks[0].text, "hecha a mano");
    }

    #[test]
    fn sin_cabecera_es_error_claro() {
        for md in ["", "hola", "## Objetivo\n", "#\n"] {
            assert_eq!(
                FeatureDoc::parse(md),
                Err(OddError::MissingHeader),
                "entrada {md:?}"
            );
        }
    }

    #[test]
    fn cabecera_sin_nombre_es_error_de_linea() {
        assert!(matches!(
            FeatureDoc::parse("# \n"),
            Err(OddError::Malformed { line: 1, .. })
        ));
    }

    #[test]
    fn cadena_de_un_megabyte_se_rechaza_sin_procesarla() {
        let s = "x".repeat(1024 * 1024);
        assert_eq!(
            FeatureDoc::parse(&s),
            Err(OddError::TooLarge {
                size: 1024 * 1024,
                max: MAX_DOC_BYTES
            })
        );
    }

    #[test]
    fn tamano_exacto_se_acepta_y_uno_mas_se_rechaza() {
        let prefijo = "# f\n\n## Objetivo\n\n";
        let relleno = "a".repeat(MAX_DOC_BYTES - prefijo.len());
        let ok = format!("{prefijo}{relleno}");
        assert_eq!(ok.len(), MAX_DOC_BYTES);
        assert_eq!(
            FeatureDoc::parse(&ok).unwrap().objective.len(),
            relleno.len()
        );

        let demasiado = format!("{ok}a");
        assert_eq!(
            FeatureDoc::parse(&demasiado),
            Err(OddError::TooLarge {
                size: MAX_DOC_BYTES + 1,
                max: MAX_DOC_BYTES
            })
        );
    }

    #[test]
    fn sesenta_tareas_se_aceptan_y_sesenta_y_una_no() {
        let construir = |n: usize| {
            let mut s = String::from("# f\n\n## Tareas\n\n");
            for i in 1..=n {
                s.push_str(&format!("- [ ] T{i} tarea {i}\n"));
            }
            s
        };
        assert_eq!(
            FeatureDoc::parse(&construir(MAX_TASKS))
                .unwrap()
                .tasks
                .len(),
            MAX_TASKS
        );
        assert_eq!(
            FeatureDoc::parse(&construir(MAX_TASKS + 1)),
            Err(OddError::TooManyTasks {
                count: MAX_TASKS + 1,
                max: MAX_TASKS
            })
        );
    }

    #[test]
    fn identificadores_duplicados_son_error() {
        let md = "# f\n\n## Tareas\n\n- [ ] T1 a\n- [x] T1 b — evidencia: c\n";
        assert_eq!(
            FeatureDoc::parse(md),
            Err(OddError::DuplicateTask { id: "T1".into() })
        );
    }

    #[test]
    fn texto_de_tarea_desmesurado_es_error() {
        let texto = "a".repeat(MAX_TASK_TEXT_BYTES);
        let ok = format!("# f\n\n## Tareas\n\n- [ ] T1 {texto}\n");
        assert!(FeatureDoc::parse(&ok).is_ok());

        let largo = "a".repeat(MAX_TASK_TEXT_BYTES + 1);
        let md = format!("# f\n\n## Tareas\n\n- [ ] T1 {largo}\n");
        assert_eq!(
            FeatureDoc::parse(&md),
            Err(OddError::TaskFieldTooLong {
                id: "T1".into(),
                field: "texto",
                size: MAX_TASK_TEXT_BYTES + 1,
                max: MAX_TASK_TEXT_BYTES,
            })
        );
    }

    #[test]
    fn evidencia_y_motivo_desmesurados_son_error() {
        let evidencia = "e".repeat(MAX_EVIDENCE_BYTES + 1);
        let md = format!("# f\n\n## Tareas\n\n- [x] T1 t — evidencia: {evidencia}\n");
        assert!(matches!(
            FeatureDoc::parse(&md),
            Err(OddError::TaskFieldTooLong {
                field: "evidencia",
                ..
            })
        ));

        let motivo = "m".repeat(MAX_TASK_TEXT_BYTES + 1);
        let md = format!("# f\n\n## Tareas\n\n- [!] T1 t — bloqueada: {motivo}\n");
        assert!(matches!(
            FeatureDoc::parse(&md),
            Err(OddError::TaskFieldTooLong {
                field: "motivo",
                ..
            })
        ));
    }

    #[test]
    fn linea_de_tarea_mal_formada_es_error_con_numero_de_linea() {
        let md = "# f\n\n## Tareas\n\n- [z] T1 x\n";
        assert!(matches!(
            FeatureDoc::parse(md),
            Err(OddError::Malformed { line: 5, .. })
        ));
        assert!(matches!(
            FeatureDoc::parse("# f\n\n## Tareas\n\n- [ ] T1\n"),
            Err(OddError::Malformed { line: 5, .. })
        ));
    }

    #[test]
    fn registro_conserva_solo_las_ultimas_100_lineas() {
        let mut md = String::from("# f\n\n## Registro\n\n");
        for i in 1..=150 {
            md.push_str(&format!("- linea {i}\n"));
        }
        let doc = FeatureDoc::parse(&md).unwrap();
        assert_eq!(doc.log.len(), MAX_LOG_LINES);
        assert_eq!(doc.log.first().map(String::as_str), Some("linea 51"));
        assert_eq!(doc.log.last().map(String::as_str), Some("linea 150"));
    }

    #[test]
    fn basura_aleatoria_nunca_entra_en_panico() {
        // Generador congruencial fijo: la prueba es determinista y no necesita crates nuevos.
        let fragmentos = [
            "# ",
            "# f\n",
            "## ",
            "## Tareas",
            "## Registro",
            "## Objetivo",
            "## Siguiente paso",
            "- [ ] T1 ",
            "- [x] T2 ",
            "- [X] T3 ",
            "- [!] T4 ",
            " — evidencia: ",
            " — bloqueada: ",
            "\n",
            "\n",
            "\r\n",
            "ñ",
            "漢",
            "😀",
            "—",
            "[",
            "]",
            "-",
            " ",
            "x",
            "T",
            "9",
            "\u{0}",
            "\t",
        ];
        let mut semilla: u64 = 0x2026_1107;
        let mut siguiente = || {
            semilla = semilla
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (semilla >> 33) as usize
        };
        for _ in 0..2000 {
            let mut s = String::new();
            if siguiente() % 2 == 0 {
                s.push_str("# f\n\n");
            }
            for _ in 0..(siguiente() % 40) {
                s.push_str(fragmentos[siguiente() % fragmentos.len()]);
            }
            // Si la entrada se acepta, el documento debe sobrevivir a su propia serialización.
            if let Ok(doc) = FeatureDoc::parse(&s) {
                assert_eq!(
                    FeatureDoc::parse(&doc.to_markdown()),
                    Ok(doc),
                    "entrada {s:?}"
                );
            }
        }
    }

    #[test]
    fn tareas_con_bloqueo_y_registro_sobreviven_a_la_vuelta() {
        let doc = FeatureDoc {
            feature: "f".into(),
            tasks: vec![Task {
                id: "T1".into(),
                text: "sin motivo".into(),
                status: TaskStatus::Blocked,
                evidence: None,
                reason: None,
            }],
            log: vec!["una línea".into()],
            ..FeatureDoc::default()
        };
        assert_eq!(FeatureDoc::parse(&doc.to_markdown()), Ok(doc));
    }
}
