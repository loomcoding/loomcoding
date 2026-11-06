//! Bloque «Ficheros» del menú izquierdo: una fila por fichero tocado en la sesión y, debajo del
//! desplegado, las líneas de su diff.
//!
//! Cabe señalar que las cuentas `+A −B` salen de `App::changes`, de modo que se ven aunque `git`
//! no esté disponible; el desglose sale de `App::session_diff`.

use super::*;
use crate::session_diff::{DiffLine, DiffOutcome, FileDiff, FileKind};
use loomcoding_cloth::state::{FileChange, StyleToken};
use loomcoding_cloth::widgets::Theme;

/// Líneas de diff que se muestran de un fichero desplegado, cabeceras de hunk incluidas.
pub(super) const MAX_DIFF_LINES: usize = 12;

/// Altura que pide el bloque: el título, una fila por fichero y las líneas del desplegado. Cero
/// si no hay ficheros, de modo que sin ellos no hay ni título.
pub(super) fn files_block_height(app: &App) -> u16 {
    if app.changes.is_empty() {
        return 0;
    }
    // El número de líneas no depende del ancho; un ancho de 1 evita trabajo de recorte.
    let (lineas, _) = filas(app, 1);
    u16::try_from(1 + lineas.len()).unwrap_or(u16::MAX)
}

/// Pinta el bloque en `area`: el título «Ficheros» y, debajo, las filas. Si no caben, se
/// desplazan para que la seleccionada quede a la vista.
pub(super) fn draw_files_block(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::LEFT);
    let interior = block.inner(area);
    frame.render_widget(block, area);
    if interior.width == 0 || interior.height == 0 {
        return;
    }
    let [titulo, cuerpo] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(interior);
    let estilo_titulo = app
        .theme
        .style(StyleToken::Heading)
        .add_modifier(Modifier::BOLD);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("Ficheros", estilo_titulo))),
        titulo,
    );
    if cuerpo.height == 0 {
        return;
    }
    let (lineas, seleccionada) = filas(app, usize::from(cuerpo.width));
    let visibles = usize::from(cuerpo.height);
    let desplazamiento = (seleccionada + 1).saturating_sub(visibles);
    let desplazamiento = u16::try_from(desplazamiento).unwrap_or(u16::MAX);
    frame.render_widget(Paragraph::new(lineas).scroll((desplazamiento, 0)), cuerpo);
}

/// Filas del cuerpo tal como se pintan: la de cada fichero y, debajo de la desplegada, su diff.
/// Devuelve también el índice de la fila del fichero seleccionado.
fn filas(app: &App, ancho_util: usize) -> (Vec<Line<'static>>, usize) {
    let mut lineas = Vec::new();
    let mut seleccionada = 0;
    for (indice, cambio) in app.changes.iter().enumerate() {
        let desplegado = app.session_diff.expanded == Some(indice);
        let seleccionado = indice == app.session_diff.selected;
        if seleccionado {
            seleccionada = lineas.len();
        }
        lineas.push(fila_de_fichero(
            cambio,
            desplegado,
            seleccionado,
            ancho_util,
            &app.theme,
        ));
        if desplegado {
            lineas.extend(cuerpo_del_diff(app, cambio, ancho_util));
        }
    }
    (lineas, seleccionada)
}

/// Fila de un fichero: marca, nombre (sin directorios) y `+A −B`.
fn fila_de_fichero(
    cambio: &FileChange,
    desplegado: bool,
    seleccionado: bool,
    ancho_util: usize,
    theme: &Theme,
) -> Line<'static> {
    let marca = if desplegado { "▾" } else { "▸" };
    let prefijo = format!("{marca} ");
    let cuenta_añadidas = format!(" +{}", cambio.added);
    let cuenta_quitadas = format!(" −{}", cambio.removed);
    let para_nombre = ancho_util
        .saturating_sub(ancho(&prefijo) + ancho(&cuenta_añadidas) + ancho(&cuenta_quitadas));
    let nombre = recorta(nombre_de(&cambio.path), para_nombre);
    // La selección solo cambia el fondo: cada trozo conserva su color.
    let estilo = |base: Style| {
        if seleccionado {
            base.patch(theme.styles.selection)
        } else {
            base
        }
    };
    Line::from(vec![
        Span::styled(prefijo, estilo(theme.styles.default)),
        Span::styled(nombre, estilo(theme.styles.tool_path)),
        Span::styled(cuenta_añadidas, estilo(theme.styles.success)),
        Span::styled(cuenta_quitadas, estilo(theme.styles.error)),
    ])
}

/// Líneas bajo la fila de un fichero desplegado: el diff, o el aviso que explica su ausencia.
fn cuerpo_del_diff(app: &App, cambio: &FileChange, ancho_util: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let aviso = |texto: &str| {
        vec![Line::from(Span::styled(
            recorta(texto, ancho_util),
            theme.styles.muted,
        ))]
    };
    match &app.session_diff.outcome {
        None => aviso("…"),
        Some(DiffOutcome::NoGit) => aviso("sin git"),
        Some(DiffOutcome::TimedOut) => aviso("git tardó demasiado"),
        Some(DiffOutcome::Ok(ficheros)) => {
            match ficheros
                .iter()
                .find(|fichero| misma_ruta(&fichero.path, &cambio.path))
            {
                None => aviso("sin diff"),
                Some(fichero) if fichero.kind == FileKind::Binary => aviso("binario"),
                Some(fichero) => lineas_del_diff(fichero, ancho_util, theme),
            }
        }
    }
}

/// Las líneas de todos los hunks, con su estilo, como mucho `MAX_DIFF_LINES`; si sobran, un
/// aviso con su número.
fn lineas_del_diff(fichero: &FileDiff, ancho_util: usize, theme: &Theme) -> Vec<Line<'static>> {
    let total: usize = fichero.hunks.iter().map(|hunk| 1 + hunk.lines.len()).sum();
    let (muted, success, error) = (theme.styles.muted, theme.styles.success, theme.styles.error);
    let items = fichero.hunks.iter().flat_map(move |hunk| {
        let cabecera = (muted, hunk.header.clone());
        let cuerpo = hunk.lines.iter().map(move |linea| match linea {
            DiffLine::Context(texto) => (muted, format!(" {texto}")),
            DiffLine::Added(texto) => (success, format!("+{texto}")),
            DiffLine::Removed(texto) => (error, format!("-{texto}")),
        });
        std::iter::once(cabecera).chain(cuerpo)
    });
    let mut lineas: Vec<Line<'static>> = items
        .take(MAX_DIFF_LINES)
        .map(|(estilo, texto)| Line::from(Span::styled(recorta(&texto, ancho_util), estilo)))
        .collect();
    if total > MAX_DIFF_LINES {
        lineas.push(Line::from(Span::styled(
            format!("… {} líneas más", total - MAX_DIFF_LINES),
            muted,
        )));
    }
    lineas
}

/// Indica si la ruta del diff corresponde al fichero de la sesión: igual, o una de las dos es el
/// sufijo de la otra a partir de un separador de directorio.
fn misma_ruta(del_diff: &str, del_cambio: &str) -> bool {
    del_diff == del_cambio
        || del_diff.ends_with(&format!("/{del_cambio}"))
        || del_cambio.ends_with(&format!("/{del_diff}"))
}

/// Solo el nombre del fichero, sin directorios; acepta `/` y `\`.
fn nombre_de(ruta: &str) -> &str {
    ruta.rsplit(['/', '\\']).next().unwrap_or(ruta)
}

/// Ancho de visualización de `texto`, en columnas de terminal.
fn ancho(texto: &str) -> usize {
    Span::raw(texto.to_string()).width()
}

/// Recorta `texto` a `max` columnas; si recorta, lo cierra con `…`.
fn recorta(texto: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if ancho(texto) <= max {
        return texto.to_string();
    }
    let mut salida = String::new();
    let mut usado = 0;
    for caracter in texto.chars() {
        let anchura = ancho(&caracter.to_string());
        if usado + anchura > max - 1 {
            break;
        }
        salida.push(caracter);
        usado += anchura;
    }
    salida.push('…');
    salida
}
