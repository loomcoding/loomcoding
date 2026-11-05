//! Tarjeta de agentes del layout `Cards`: una fila por subagente de la sesión abierta, sobre el
//! compositor. Cabe señalar que solo existe en ese layout; en los demás no reserva filas ni se pinta.

use crate::app::{agent_tag, App};
use crate::rail::{agent_state, format_elapsed, format_tokens};
use loomcoding_cloth::widgets::LayoutStyle;
use loomcoding_core::session::SubagentDescriptor;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::Instant;

/// Máximo de filas de agentes que se muestran; el resto se resume en `… N more`.
const MAX_FILAS: usize = 8;

/// Subagentes de la sesión abierta, en el orden de su identificador de hijo.
fn subagentes(app: &App) -> impl Iterator<Item = &SubagentDescriptor> + '_ {
    app.subagents
        .values()
        .filter(move |subagent| Some(subagent.parent) == app.session_id)
}

/// Tope de filas para un transcript de `alto` filas: el 25 % del alto, entre una y `MAX_FILAS`.
fn tope(alto: u16) -> usize {
    (usize::from(alto) / 4).clamp(1, MAX_FILAS)
}

/// Subagentes de la sesión y cuántos se muestran para un área de `alto` filas.
fn visibles(app: &App, alto: u16) -> (usize, usize) {
    let total = subagentes(app).count();
    (total, total.min(tope(alto)))
}

/// Filas que ocupa la tarjeta en un transcript de `area_height` filas: 0 fuera de `Cards` o sin
/// subagentes; 1 si está plegada; si no, la cabecera, las filas mostradas y, si sobran, `… N more`.
pub(super) fn card_rows(app: &App, area_height: u16) -> u16 {
    if app.theme.look.layout != LayoutStyle::Cards {
        return 0;
    }
    let (total, mostrar) = visibles(app, area_height);
    if total == 0 {
        0
    } else if app.agents_card_folded {
        1
    } else {
        let resto = u16::from(total > mostrar);
        1 + mostrar as u16 + resto
    }
}

/// Una fila de la tarjeta, ya convertida en textos y estilos.
struct Fila {
    estado: Style,
    glifo: &'static str,
    nombre: String,
    modelo: String,
    tokens: String,
    tiempo: String,
}

/// Anchura de cada columna alineada: la de lo más ancho que se muestra.
#[derive(Debug, Default, PartialEq)]
struct Anchos {
    nombre: usize,
    modelo: usize,
    tokens: usize,
}

fn anchos(filas: &[Fila]) -> Anchos {
    filas.iter().fold(Anchos::default(), |ancho, fila| Anchos {
        nombre: ancho.nombre.max(fila.nombre.chars().count()),
        modelo: ancho.modelo.max(fila.modelo.chars().count()),
        tokens: ancho.tokens.max(fila.tokens.chars().count()),
    })
}

/// Filas de los `mostrar` primeros subagentes. El modelo y el esfuerzo son los de la sesión: el
/// subagente no guarda los suyos en su descriptor. Sin tokens o sin arranque, se muestra `—`.
fn filas(app: &App, ahora: Instant, mostrar: usize) -> Vec<Fila> {
    let esfuerzo = app
        .provider_settings
        .as_ref()
        .and_then(|settings| settings.effort.as_deref())
        .unwrap_or("—");
    let modelo = format!("{} · {esfuerzo}", app.model);
    subagentes(app)
        .take(mostrar)
        .map(|subagent| {
            let stats = app.agent_stats.get(&subagent.child.0);
            let estado = agent_state(subagent.status.state);
            let nombre = match agent_tag(subagent) {
                Some(etiqueta) => format!("{etiqueta} {}", subagent.label),
                None => subagent.label.clone(),
            };
            let tokens = stats
                .map(|stats| stats.input_tokens)
                .filter(|&entrada| entrada > 0)
                .map_or_else(|| "—".to_owned(), format_tokens);
            let tiempo = match stats
                .and_then(|stats| stats.started_at.map(|inicio| (inicio, stats.finished_at)))
            {
                Some((inicio, fin)) => {
                    format_elapsed(fin.unwrap_or(ahora).saturating_duration_since(inicio))
                }
                None => "—".to_owned(),
            };
            Fila {
                estado: estado.style(&app.theme.styles),
                glifo: estado.glyph(),
                nombre,
                modelo: modelo.clone(),
                tokens,
                tiempo,
            }
        })
        .collect()
}

/// Cabecera: ` ❀ Agentes` a la izquierda y, a la derecha, la acción del pliegue.
fn cabecera(app: &App, ancho: u16, plegada: bool) -> Line<'static> {
    let accion = if plegada {
        "▸ desplegar "
    } else {
        "▾ plegar "
    };
    let usado = " ❀ Agentes".chars().count() + accion.chars().count();
    let hueco = usize::from(ancho).saturating_sub(usado);
    Line::from(vec![
        Span::styled(" ❀ ", app.theme.styles.accent),
        Span::styled(
            "Agentes",
            app.theme.styles.default.add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(hueco)),
        Span::styled(accion, app.theme.styles.muted),
    ])
}

/// Una fila de agente: glifo de estado y nombre, y a su derecha las columnas alineadas.
fn linea(fila: &Fila, ancho: &Anchos, app: &App) -> Line<'static> {
    let tenue = app.theme.styles.muted;
    Line::from(vec![
        Span::styled(format!(" {} ", fila.glifo), fila.estado),
        Span::styled(
            format!("{:<w$}", fila.nombre, w = ancho.nombre),
            app.theme.styles.default,
        ),
        Span::styled(format!("  {:<w$}", fila.modelo, w = ancho.modelo), tenue),
        Span::styled(format!("  {:<w$}", fila.tokens, w = ancho.tokens), tenue),
        Span::styled(format!("  {}", fila.tiempo), tenue),
    ])
}

/// Pinta la tarjeta en `area`, que tiene las filas que dio `card_rows`. `alto` es el alto del
/// área del transcript con el que se calculó ese número, para que el tope coincida.
pub(super) fn draw_card(frame: &mut Frame, area: Rect, app: &App, alto: u16) {
    if area.is_empty() {
        return;
    }
    let plegada = app.agents_card_folded;
    let (total, mostrar) = visibles(app, alto);
    let mut lineas = vec![cabecera(app, area.width, plegada)];
    if !plegada {
        let filas = filas(app, Instant::now(), mostrar);
        let ancho = anchos(&filas);
        lineas.extend(filas.iter().map(|fila| linea(fila, &ancho, app)));
        if total > mostrar {
            lineas.push(Line::from(Span::styled(
                format!(" … {} more", total - mostrar),
                app.theme.styles.muted,
            )));
        }
    }
    for (i, linea) in lineas
        .into_iter()
        .take(usize::from(area.height))
        .enumerate()
    {
        let fila = Rect::new(area.x, area.y + i as u16, area.width, 1);
        frame.render_widget(Paragraph::new(linea), fila);
    }
}
