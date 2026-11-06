//! Caja redonda del mensaje del usuario en el carril `box` (estilo MastraCode).
//!
//! Solo el mensaje del usuario lleva trazo: `╭─╮` sobre su fila de etiqueta, `│` a ambos lados
//! del cuerpo y `╰─╯` en la fila extra que cloth reserva al final de la entrada. El texto se mide
//! sin las dos columnas de la derecha, así que el borde nunca lo pisa. Con foco, el trazo pasa a
//! grueso (`┏━┓ ┃ ┗━┛`). Solo se tocan celdas de la caja: no se re-renderiza nada.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// Columnas que el transcript deja libres a la derecha del texto cuando hay caja: la primera es
/// el relleno y la última, el borde.
pub(super) const RESERVA_DERECHA: u16 = 2;

/// Símbolos de una caja: esquinas, horizontal y vertical.
#[derive(Clone, Copy)]
struct Trazo {
    arriba_izq: char,
    arriba_der: char,
    abajo_izq: char,
    abajo_der: char,
    horizontal: char,
    vertical: char,
}

const FINO: Trazo = Trazo {
    arriba_izq: '╭',
    arriba_der: '╮',
    abajo_izq: '╰',
    abajo_der: '╯',
    horizontal: '─',
    vertical: '│',
};

const GRUESO: Trazo = Trazo {
    arriba_izq: '┏',
    arriba_der: '┓',
    abajo_izq: '┗',
    abajo_der: '┛',
    horizontal: '━',
    vertical: '┃',
};

/// Lo necesario para pintar la caja de una entrada del usuario.
pub(super) struct Caja {
    /// Columna del borde izquierdo: la primera del carril.
    pub(super) x0: u16,
    /// Área del texto de la entrada. Su columna siguiente es el relleno y la de después, el borde.
    pub(super) texto: Rect,
    /// Filas absolutas de la entrada: la de etiqueta y la siguiente a la última (exclusiva).
    pub(super) inicio: usize,
    pub(super) fin: usize,
    /// Bloque enfocado: el trazo pasa a grueso.
    pub(super) enfocado: bool,
    /// Color del trazo: el acento del rol, o el del bloque enfocado.
    pub(super) estilo: Style,
    /// Prompt del rol, que va en la primera fila de texto.
    pub(super) glifo: char,
    /// Hay barra de desplazamiento en la última columna de `texto`: la caja no la pisa.
    pub(super) con_barra: bool,
}

/// Pinta la caja en las filas visibles `[top, bottom)` del búfer. La fila `top` cae en la fila
/// `texto.y` de la pantalla; las filas fuera de la ventana se omiten sin error.
pub(super) fn pinta_caja(buf: &mut Buffer, caja: &Caja, top: usize, bottom: usize) {
    let trazo = if caja.enfocado { GRUESO } else { FINO };
    let texto = caja.texto;
    let borde_der = texto.right().saturating_add(1);
    let barra = caja.con_barra.then(|| texto.right().saturating_sub(1));
    for fila in caja.inicio.max(top)..caja.fin.min(bottom) {
        let Ok(dy) = u16::try_from(fila - top) else {
            break;
        };
        let y = texto.y + dy;
        if fila == caja.inicio || fila + 1 == caja.fin {
            let (izq, der) = if fila == caja.inicio {
                (trazo.arriba_izq, trazo.arriba_der)
            } else {
                (trazo.abajo_izq, trazo.abajo_der)
            };
            celda(buf, caja.x0, y, izq, caja.estilo);
            // La horizontal llega hasta la columna del relleno; la barra de desplazamiento, si
            // existe, se queda como está.
            for x in caja.x0 + 1..=texto.right() {
                if Some(x) != barra {
                    celda(buf, x, y, trazo.horizontal, caja.estilo);
                }
            }
            celda(buf, borde_der, y, der, caja.estilo);
        } else {
            celda(buf, caja.x0, y, trazo.vertical, caja.estilo);
            let prompt = if fila == caja.inicio + 1 {
                caja.glifo
            } else {
                ' '
            };
            celda(buf, caja.x0 + 2, y, prompt, caja.estilo);
            celda(buf, borde_der, y, trazo.vertical, caja.estilo);
        }
    }
}

/// Fija el símbolo y el estilo de una celda, si cae dentro del búfer.
fn celda(buf: &mut Buffer, x: u16, y: u16, simbolo: char, estilo: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_symbol(simbolo.encode_utf8(&mut [0; 4]))
            .set_style(estilo);
    }
}
