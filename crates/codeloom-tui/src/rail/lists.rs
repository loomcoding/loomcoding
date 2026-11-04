//! Listas acotadas: tareas, ficheros, herramientas y trabajos.

use super::*;

/// Lista de tareas con su progreso.
pub(super) fn push_todos<'a>(out: &mut Vec<Line<'a>>, app: &'a App) {
    if !app.todos.is_empty() {
        push_gap(out);
        let done = app
            .todos
            .iter()
            .filter(|todo| todo.status == TodoStatus::Completed)
            .count();
        out.push(Line::from(vec![
            Span::styled(
                "TODOS",
                app.theme.styles.accent.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {done}/{}", app.todos.len()),
                app.theme.styles.muted,
            ),
        ]));
        for todo in app.todos.iter().take(TODO_LIMIT) {
            let (glyph, style) = match todo.status {
                TodoStatus::Pending => ('☐', app.theme.styles.todo_pending),
                TodoStatus::InProgress => ('◐', app.theme.styles.todo_in_progress),
                TodoStatus::Completed => ('☑', app.theme.styles.todo_completed),
            };
            // Una sola línea: los saltos de línea romperían el recorte del rail.
            let content = todo
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            out.push(Line::from(vec![
                Span::styled(format!("{glyph} "), style),
                Span::raw(content),
            ]));
        }
        more(out, app.todos.len(), TODO_LIMIT, app);
    }
}

/// Ficheros cambiados.
pub(super) fn push_files<'a>(out: &mut Vec<Line<'a>>, app: &'a App) {
    if !app.changes.is_empty() {
        push_gap(out);
        out.push(title("FILES", app));
        for change in app.changes.iter().take(FILE_LIMIT) {
            out.push(Line::from(vec![
                Span::styled("● ", app.theme.styles.success),
                Span::raw(change.path.as_str()),
                Span::styled(format!("  +{}", change.added), app.theme.styles.success),
                Span::styled(format!("  -{}", change.removed), app.theme.styles.error),
            ]));
        }
        more(out, app.changes.len(), FILE_LIMIT, app);
    }
}

/// Herramientas recientes.
pub(super) fn push_tools<'a>(out: &mut Vec<Line<'a>>, app: &'a App) {
    if !app.tools.is_empty() {
        push_gap(out);
        out.push(title("TOOLS", app));
        let skip = app.tools.len().saturating_sub(TOOL_LIMIT);
        for tool in app.tools.iter().skip(skip) {
            out.push(glyph_line(
                tool.name.as_str(),
                tool_style(tool.status(), app),
            ));
        }
    }
}

/// Trabajos de la sesión que no son subagentes.
pub(super) fn push_jobs<'a>(out: &mut Vec<Line<'a>>, app: &'a App) {
    let session = app.session_id;
    let jobs = || {
        app.jobs
            .values()
            .filter(move |job| Some(job.owner) == session && job.kind != "subagent")
    };
    let job_total = jobs().count();
    if job_total > 0 {
        push_gap(out);
        out.push(title("JOBS", app));
        for job in jobs().take(JOB_LIMIT) {
            let style = match job.status {
                JobStatus::Running | JobStatus::Stopping => app.theme.styles.tool_running,
                JobStatus::Completed => app.theme.styles.tool_success,
                JobStatus::Failed => app.theme.styles.tool_error,
                JobStatus::Killed => app.theme.styles.tool_canceled,
            };
            out.push(glyph_line(job.label.as_str(), style));
        }
        more(out, job_total, JOB_LIMIT, app);
    }
}
