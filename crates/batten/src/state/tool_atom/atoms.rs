//! Átomos por familia de herramienta (primaria, fichero, búsqueda, pregunta, control y lista) y sus instancias.

use super::*;

/// Pieza de una llamada cuyo resumen es un solo dato: el comando de `bash` y
/// `job_start`, o la etiqueta de `subagent` y el id de `job_output`/`job_kill`.
/// La salida plegada va como cuerpo.
pub(super) struct PrimaryAtom {
    pub(super) part: Part,
}

impl ToolAtom for PrimaryAtom {
    fn lines(&self, item: &ToolItem, facts: &ToolFacts) -> Vec<Line> {
        let Some(primary) = facts.primary.as_deref().and_then(first_line) else {
            return tool_lines(item);
        };
        with_body(item, &with_tail(primary, self.part, None))
    }
}

/// Pieza de `view`: la ruta y, si lo hay, el rango de líneas.
pub(super) struct FileAtom;

impl ToolAtom for FileAtom {
    fn lines(&self, item: &ToolItem, facts: &ToolFacts) -> Vec<Line> {
        let Some(path) = facts.primary.as_deref().and_then(first_line) else {
            return tool_lines(item);
        };
        let range = facts.range.map(|(start, end)| format!("L{start}-{end}"));
        with_body(item, &with_tail(path, Part::Path, range))
    }
}

/// Pieza de búsquedas (`grep`, `glob`). `label` nombra lo contado: «matches»
/// para coincidencias o «files» para archivos.
pub(super) struct SearchAtom {
    pub(super) label: &'static str,
}

impl ToolAtom for SearchAtom {
    fn lines(&self, item: &ToolItem, facts: &ToolFacts) -> Vec<Line> {
        let Some(pattern) = facts.primary.as_deref().and_then(first_line) else {
            return tool_lines(item);
        };
        let filter = facts
            .secondary
            .as_deref()
            .and_then(first_line)
            .map(|glob| format!("in {glob}"));
        let mut lines = with_body(item, &with_tail(pattern, Part::Pattern, filter));
        // Sin recuento explícito, se cuentan las líneas de la salida. Sin salida
        // aún (en curso) no hay recuento que mostrar.
        let count = facts
            .count
            .or_else(|| item.output.as_deref().map(count_nonempty));
        if let Some(count) = count {
            lines.push(count_line(count, self.label));
        }
        lines
    }
}

/// Pieza de `ask_user`: la primera pregunta y, si la hay, su número de opciones.
pub(super) struct AskAtom;

impl ToolAtom for AskAtom {
    fn lines(&self, item: &ToolItem, facts: &ToolFacts) -> Vec<Line> {
        let Some(question) = facts.primary.as_deref().and_then(first_line) else {
            return tool_lines(item);
        };
        let question = truncate_graphemes(question, SUMMARY_GRAPHEMES);
        let mut head = head_line(item, &[(Part::Text, question)]);
        if let Some(options) = facts.count {
            // Dos espacios separan la pregunta del recuento, como en `edit`.
            head.text.push_str("  ");
            push_text(&mut head, &format!("{options} options"), Part::Count);
        }
        let mut lines = vec![head];
        lines.extend(body_lines(item));
        lines
    }
}

/// Pieza de `subagent_control`: la acción y, tras un espacio, el id del hijo.
pub(super) struct ControlAtom;

impl ToolAtom for ControlAtom {
    fn lines(&self, item: &ToolItem, facts: &ToolFacts) -> Vec<Line> {
        let Some(action) = facts.primary.as_deref().and_then(first_line) else {
            return tool_lines(item);
        };
        let target = facts
            .secondary
            .as_deref()
            .and_then(first_line)
            .map(str::to_string);
        with_body(item, &with_tail(action, Part::Text, target))
    }
}

/// Pieza de los listados (`subagent_list`, `job_list`): «N items» en la cabecera.
/// El recuento es el explícito de los hechos o, si no lo hay, las líneas no
/// vacías de la salida (si hay más de una); en curso y sin salida no hay recuento.
pub(super) struct ListAtom;

impl ToolAtom for ListAtom {
    fn lines(&self, item: &ToolItem, facts: &ToolFacts) -> Vec<Line> {
        let count = facts.count.or_else(|| {
            // Una salida de una sola línea (p. ej. un JSON compacto) no
            // permite deducir cuántos elementos contiene: sin recuento.
            item.output
                .as_deref()
                .map(count_nonempty)
                .filter(|&n| n > 1)
        });
        let parts: Vec<(Part, String)> = count
            .map(|count| vec![(Part::Count, format!("{count} items"))])
            .unwrap_or_default();
        with_body(item, &parts)
    }
}

pub(super) static SHELL: PrimaryAtom = PrimaryAtom {
    part: Part::Command,
};

pub(super) static REFERENCE: PrimaryAtom = PrimaryAtom { part: Part::Text };

pub(super) static FILE: FileAtom = FileAtom;

pub(super) static EDIT: EditAtom = EditAtom;

pub(super) static GREP: SearchAtom = SearchAtom { label: "matches" };

pub(super) static GLOB: SearchAtom = SearchAtom { label: "files" };

pub(super) static ASK: AskAtom = AskAtom;

pub(super) static CONTROL: ControlAtom = ControlAtom;

pub(super) static LIST: ListAtom = ListAtom;
