//! Medida de la latencia del composer: lo que cuesta pulsar una tecla y ver el resultado pintado.
//!
//! Cabe señalar que la TUI procesa la entrada y pinta en la misma tarea, de modo que el coste de
//! un frame es, de hecho, la latencia de la tecla siguiente. Estas pruebas miden ese coste con
//! conversaciones de distinto tamaño. El informe va con `#[ignore]` y se lanza a mano:
//! `cargo test -p codeloom-tui --test composer_latency --release -- --ignored --nocapture`.

use std::time::{Duration, Instant};

use codeloom_tui::app::App;
use codeloom_tui::render::render;
use codeloom_tui::run::handle_event;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

const ANCHO: u16 = 120;
const ALTO: u16 = 40;

const PARRAFO: &str = "Cabe señalar que el **bucle de eventos** procesa la entrada y pinta en la misma \
tarea, de modo que cada frame cuenta. Véase `render.rs` y la lista:\n\n- uno con `código`\n- dos con \
[enlace](https://example.com)\n- tres con **negrita** y _cursiva_\n\n```rust\nfn main() {\n    \
println!(\"hola, mundo\");\n}\n```\n\nY un párrafo final bastante más largo para que ocupe varias \
líneas al ajustarse al ancho de la terminal y obligue a medir y a partir texto un buen rato.";

fn conversacion(mensajes: usize) -> App {
    let mut app = App::new();
    for i in 0..mensajes {
        app.push_user(&format!(
            "Pregunta número {i}: ¿cómo va la medición del composer?"
        ));
        app.push_assistant(PARRAFO);
    }
    app
}

fn tecla(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
}

fn percentil(mut muestras: Vec<Duration>, p: f64) -> Duration {
    muestras.sort();
    muestras[((muestras.len() as f64 - 1.0) * p).round() as usize]
}

fn resumen(nombre: &str, muestras: &[Duration]) {
    println!(
        "{nombre:<46} p50 {:>8.2?}  p95 {:>8.2?}  max {:>8.2?}",
        percentil(muestras.to_vec(), 0.50),
        percentil(muestras.to_vec(), 0.95),
        percentil(muestras.to_vec(), 1.0),
    );
}

/// Pinta `veces` frames tras cada tecla y devuelve el coste de «tecla + frame».
fn teclas_con_frame(
    app: &mut App,
    term: &mut Terminal<TestBackend>,
    veces: usize,
) -> Vec<Duration> {
    (0..veces)
        .map(|i| {
            let c = (b'a' + (i % 26) as u8) as char;
            let inicio = Instant::now();
            handle_event(app, tecla(c));
            term.draw(|f| render(f, app)).unwrap();
            inicio.elapsed()
        })
        .collect()
}

#[test]
#[ignore = "informe de latencia: se lanza a mano con --release --nocapture"]
fn report_keystroke_to_frame_latency() {
    let mut term = Terminal::new(TestBackend::new(ANCHO, ALTO)).unwrap();
    println!(
        "\n== tecla + frame ({ANCHO}x{ALTO}, perfil {}) ==",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    for mensajes in [0, 50, 500, 3000] {
        let mut app = conversacion(mensajes);
        // El primer frame llena cachés: no cuenta como pulsación.
        term.draw(|f| render(f, &mut app)).unwrap();
        let muestras = teclas_con_frame(&mut app, &mut term, 200);
        resumen(&format!("{mensajes} mensajes, escribiendo"), &muestras);
    }

    println!("\n== con streaming a la vez ==");
    for mensajes in [50, 500] {
        let mut app = conversacion(mensajes);
        app.push_user("pregunta en curso");
        term.draw(|f| render(f, &mut app)).unwrap();
        let muestras: Vec<Duration> = (0..200)
            .map(|i| {
                let inicio = Instant::now();
                app.push_assistant("palabra ");
                handle_event(&mut app, tecla((b'a' + (i % 26) as u8) as char));
                term.draw(|f| render(f, &mut app)).unwrap();
                inicio.elapsed()
            })
            .collect();
        resumen(&format!("{mensajes} mensajes, delta + tecla"), &muestras);
    }

    println!("\n== pegado ==");
    for bytes in [1_000usize, 20_000, 200_000] {
        let mut app = conversacion(50);
        term.draw(|f| render(f, &mut app)).unwrap();
        let texto = "línea de texto pegado con acentos ñ\n".repeat(bytes / 36);
        let inicio = Instant::now();
        handle_event(&mut app, Event::Paste(texto));
        term.draw(|f| render(f, &mut app)).unwrap();
        println!(
            "pegar {bytes:>7} bytes + frame: {:>10.2?}",
            inicio.elapsed()
        );
        let siguiente = teclas_con_frame(&mut app, &mut term, 20);
        resumen("   tecla siguiente tras pegar", &siguiente);
    }
}

fn medir<T>(nombre: &str, veces: usize, mut f: impl FnMut() -> T) {
    let muestras: Vec<Duration> = (0..veces)
        .map(|_| {
            let inicio = Instant::now();
            std::hint::black_box(f());
            inicio.elapsed()
        })
        .collect();
    resumen(nombre, &muestras);
}

#[test]
#[ignore = "desglose del coste con un texto grande pegado: se lanza a mano"]
fn report_where_a_big_paste_spends_its_time() {
    let mut term = Terminal::new(TestBackend::new(ANCHO, ALTO)).unwrap();
    let mut app = conversacion(50);
    let texto = "línea de texto pegado con acentos ñ
"
    .repeat(200_000 / 36);
    handle_event(&mut app, Event::Paste(texto));
    term.draw(|f| render(f, &mut app)).unwrap();
    println!(
        "
== desglose con ~200 KB en el composer ({}) ==",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );

    medir("handle_event(tecla)", 20, || {
        handle_event(&mut app, tecla('x'))
    });
    medir("render completo (draw)", 20, || {
        term.draw(|f| render(f, &mut app)).unwrap();
    });
    medir("composer.cursor()", 20, || app.composer.cursor());
    medir("composer.lines()", 20, || app.composer.lines());
    medir("composer_layout(texto, cursor, 118, 116)", 20, || {
        batten::state::composer_layout(app.composer.text(), app.composer.cursor(), 118, 116)
    });
    {
        let mut a = conversacion(50);
        term.draw(|f| render(f, &mut a)).unwrap();
        medir("render completo SIN texto grande", 20, || {
            term.draw(|f| render(f, &mut a)).unwrap();
        });
    }
}
