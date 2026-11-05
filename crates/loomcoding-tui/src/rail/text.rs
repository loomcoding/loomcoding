//! Piezas de texto y de línea: recorte, partido por palabras, títulos y huecos.

use super::*;

/// Parte `texto` en líneas de como mucho `width` columnas, cortando por espacios. Una palabra
/// más ancha que la línea se recorta con `…`, como en el resto del rail.
pub(crate) fn partir_por_palabras(texto: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lineas = Vec::new();
    let mut actual = String::new();
    for palabra in texto.split_whitespace() {
        let palabra = recortar(palabra, width);
        let nueva = if actual.is_empty() {
            palabra.clone()
        } else {
            format!("{actual} {palabra}")
        };
        if anchura(&nueva) <= width {
            actual = nueva;
        } else {
            lineas.push(std::mem::replace(&mut actual, palabra));
        }
    }
    if !actual.is_empty() {
        lineas.push(actual);
    }
    lineas
}

/// Título de sección: negrita con el estilo por defecto del tema, sin acento.
pub(super) fn title<'a>(text: &'a str, app: &App) -> Line<'a> {
    Line::from(Span::styled(
        text,
        app.theme.styles.default.add_modifier(Modifier::BOLD),
    ))
}

/// Línea con un glifo `●` del color indicado y la etiqueta.
pub(super) fn glyph_line<'a>(label: &'a str, style: Style) -> Line<'a> {
    Line::from(vec![Span::styled("● ", style), Span::raw(label)])
}

/// Anchura en columnas de `texto`, medida con la misma lógica que ratatui.
pub(super) fn anchura(texto: &str) -> usize {
    Span::raw(texto).width()
}

/// Recorta `texto` para que ocupe como mucho `width` columnas. Si recorta, el
/// último carácter visible es `…`, que cuenta como una columna.
pub(super) fn recortar(texto: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if anchura(texto) <= width {
        return texto.to_owned();
    }
    let mut out = String::new();
    let mut usado = 0;
    let mut buf = [0u8; 4];
    for c in texto.chars() {
        let ancho = anchura(c.encode_utf8(&mut buf));
        if usado + ancho > width - 1 {
            break;
        }
        out.push(c);
        usado += ancho;
    }
    out.push('…');
    out
}

/// Resumen `… +n more` cuando la lista tiene más elementos que el límite.
pub(super) fn more(out: &mut Vec<Line<'_>>, total: usize, limit: usize, app: &App) {
    if total > limit {
        out.push(Line::from(Span::styled(
            format!("… +{} more", total - limit),
            app.theme.styles.muted,
        )));
    }
}

/// Línea en blanco entre secciones, solo si ya hay contenido antes.
pub(super) fn push_gap(out: &mut Vec<Line<'_>>) {
    if !out.is_empty() {
        out.push(Line::default());
    }
}

pub(super) fn tool_style(status: ToolStatus, app: &App) -> Style {
    match status {
        ToolStatus::Pending => app.theme.styles.tool_pending,
        ToolStatus::Running => app.theme.styles.tool_running,
        ToolStatus::Success => app.theme.styles.tool_success,
        ToolStatus::Error => app.theme.styles.tool_error,
        ToolStatus::Canceled => app.theme.styles.tool_canceled,
        ToolStatus::AwaitingPermission => app.theme.styles.warning,
    }
}
