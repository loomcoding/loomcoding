//! Documento de feature: datos, cotas y consultas. El formato Markdown vive en `render.rs`.
//!
//! El documento es la fuente de verdad de las tareas de una feature; la lista de tareas de la
//! TUI será luego una proyección suya (tarea 7 del plan).

/// Tamaño máximo del documento completo, en bytes.
pub const MAX_DOC_BYTES: usize = 32 * 1024;
/// Número máximo de tareas por documento.
pub const MAX_TASKS: usize = 60;
/// Bytes máximos del texto de una tarea. También se aplica al motivo de un bloqueo.
pub const MAX_TASK_TEXT_BYTES: usize = 300;
/// Bytes máximos de la evidencia de una casilla marcada.
pub const MAX_EVIDENCE_BYTES: usize = 500;
/// Líneas máximas del registro; se conservan las últimas.
pub const MAX_LOG_LINES: usize = 100;
/// Bytes máximos de cada línea del registro.
pub const MAX_LOG_LINE_BYTES: usize = 300;

/// Estado de una tarea. `Blocked` cuenta como no hecha, igual que `Pending`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Done,
    Blocked,
}

/// Tarea con identificador estable (`T1`, `T2`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: String,
    pub text: String,
    pub status: TaskStatus,
    pub evidence: Option<String>,
    pub reason: Option<String>,
}

impl Task {
    /// Tarea pendiente nueva. El texto se recorta a [`MAX_TASK_TEXT_BYTES`] en frontera de carácter.
    pub fn new(id: &str, text: &str) -> Self {
        Self {
            id: id.to_string(),
            text: limit_bytes(text.trim(), MAX_TASK_TEXT_BYTES)
                .trim_end()
                .to_string(),
            status: TaskStatus::Pending,
            evidence: None,
            reason: None,
        }
    }
}

/// Documento de una feature: objetivo, motivo, alcance, restricciones, tareas, registro y
/// siguiente paso.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeatureDoc {
    pub feature: String,
    pub objective: String,
    pub why: String,
    pub scope: String,
    pub constraints: String,
    pub tasks: Vec<Task>,
    pub log: Vec<String>,
    pub next_step: String,
}

impl FeatureDoc {
    /// Tareas en estado `Pending`. Las `Blocked` no aparecen aquí; para ellas está [`FeatureDoc::blocked`].
    pub fn pending(&self) -> Vec<&Task> {
        self.tasks_with(TaskStatus::Pending)
    }

    /// Tareas en estado `Blocked`.
    pub fn blocked(&self) -> Vec<&Task> {
        self.tasks_with(TaskStatus::Blocked)
    }

    fn tasks_with(&self, status: TaskStatus) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == status).collect()
    }

    /// Añade una línea al registro. Aplana saltos de línea, descarta las vacías, acota cada
    /// línea a [`MAX_LOG_LINE_BYTES`] y conserva solo las últimas [`MAX_LOG_LINES`].
    pub fn push_log(&mut self, line: &str) {
        let plana: String = line
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        let acotada = limit_bytes(plana.trim(), MAX_LOG_LINE_BYTES).trim_end();
        if acotada.is_empty() {
            return;
        }
        self.log.push(acotada.to_string());
        if self.log.len() > MAX_LOG_LINES {
            let sobran = self.log.len() - MAX_LOG_LINES;
            self.log.drain(..sobran);
        }
    }
}

/// Devuelve el prefijo de `s` de como mucho `max` bytes, cortado en frontera de carácter.
pub fn limit_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    // El byte 0 siempre es frontera, así que el bucle termina.
    let mut fin = max;
    while !s.is_char_boundary(fin) {
        fin -= 1;
    }
    &s[..fin]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, status: TaskStatus) -> Task {
        Task {
            id: id.to_string(),
            text: format!("texto de {id}"),
            status,
            evidence: None,
            reason: None,
        }
    }

    #[test]
    fn task_new_recorta_el_texto_en_frontera_de_caracter() {
        // 299 bytes de 'a' seguidos de una 'é' (2 bytes): el corte no puede partir la 'é'.
        let texto = format!("{}é", "a".repeat(299));
        let t = Task::new("T1", &texto);
        assert_eq!(t.text, "a".repeat(299));
        assert_eq!(t.status, TaskStatus::Pending);
        assert!(t.evidence.is_none() && t.reason.is_none());
    }

    #[test]
    fn limit_bytes_no_parte_caracteres_multibyte() {
        assert_eq!(limit_bytes("😀😀", 5), "😀");
        assert_eq!(limit_bytes("😀😀", 3), "");
        assert_eq!(limit_bytes("漢字", 4), "漢");
        assert_eq!(limit_bytes("abc", 10), "abc");
    }

    #[test]
    fn push_log_conserva_solo_las_ultimas_100_lineas() {
        let mut d = FeatureDoc::default();
        for i in 0..150 {
            d.push_log(&format!("linea {i}"));
        }
        assert_eq!(d.log.len(), MAX_LOG_LINES);
        assert_eq!(d.log.first().map(String::as_str), Some("linea 50"));
        assert_eq!(d.log.last().map(String::as_str), Some("linea 149"));
    }

    #[test]
    fn push_log_acota_cada_linea_a_300_bytes() {
        let mut d = FeatureDoc::default();
        d.push_log(&"ñ".repeat(200)); // 400 bytes; cada 'ñ' ocupa 2
        let l = &d.log[0];
        assert_eq!(l.len(), MAX_LOG_LINE_BYTES);
        assert!(l.chars().all(|c| c == 'ñ'));
    }

    #[test]
    fn push_log_descarta_vacias_y_aplana_saltos() {
        let mut d = FeatureDoc::default();
        d.push_log("   ");
        assert!(d.log.is_empty());
        d.push_log("uno\ndos");
        assert_eq!(d.log, vec!["uno dos".to_string()]);
    }

    #[test]
    fn pending_y_blocked_filtran_por_estado() {
        let d = FeatureDoc {
            tasks: vec![
                task("T1", TaskStatus::Pending),
                task("T2", TaskStatus::Done),
                task("T3", TaskStatus::Blocked),
                task("T4", TaskStatus::Pending),
            ],
            ..FeatureDoc::default()
        };
        let ids = |v: Vec<&Task>| v.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(d.pending()), vec!["T1", "T4"]);
        assert_eq!(ids(d.blocked()), vec!["T3"]);
    }
}
