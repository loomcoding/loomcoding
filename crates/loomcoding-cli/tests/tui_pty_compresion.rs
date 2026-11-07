#![cfg(windows)]
//! E2E de la compresión de salida y del registro de `tracing` en la TUI real. Con `[output_filter] enabled = true`
//! y `RUST_LOG=output_filter=info`, la salida ruidosa de un `bash` llega al modelo con su cabecera de filtrado, el
//! original íntegro queda en el volcado que la cabecera anuncia y el registro del daemon recoge una línea del destino
//! `output_filter` con la regla y los bytes. Sin `enabled = true` no hay cabecera ni línea de registro.
//!
//! Como en `tui_pty_odd.rs`, el perfil es temporal (`LOOMCODING_HOME`), el proyecto es temporal y el proveedor es un
//! `wiremock` con el guion de `mock_provider_compresion`: no hay red real.

use loomcoding_e2e::{
    kill_daemons_of, mock_provider_compresion, Key, Snapshot, Term, TermConfig, Workspace,
    COMPRESION_FINAL,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wiremock::MockServer;

const BOOT: Duration = Duration::from_secs(40);
const TURN: Duration = Duration::from_secs(25);
/// Pista del composer vacío: indica que la TUI ya está lista para recibir teclas.
const READY: &str = "Describe what you want to build";
const PETICION: &str = "ejecuta el ruido";
/// Inicio de la cabecera que antepone el filtro a la salida que recibe el modelo.
const CABECERA: &str = "[filtrado:";
/// Marca que precede a la ruta del original dentro de la cabecera.
const MARCA_RUTA: &str = "el original sigue en ";
/// Destino de `tracing` que emite el filtro, y su filtro de registro.
const DESTINO: &str = "output_filter";
const RUST_LOG: &str = "output_filter=info";

struct Session {
    term: Term,
    ws: Workspace,
    provider: MockServer,
}

impl Session {
    /// Arranca la TUI con el guion de compresión. Con `filtro_activo` se escribe `[output_filter] enabled = true`.
    /// `RUST_LOG` va siempre: el caso apagado demuestra que, sin filtro, el registro no recibe nada.
    async fn start(filtro_activo: bool) -> Session {
        let provider = mock_provider_compresion().await;
        let ws = Workspace::new(&provider.uri()).unwrap();
        if filtro_activo {
            let ruta = ws.project.path().join("loomcoding.toml");
            let mut texto = std::fs::read_to_string(&ruta).unwrap();
            texto.push_str("\n[output_filter]\nenabled = true\n");
            std::fs::write(&ruta, texto).unwrap();
        }
        let mut env = ws.env();
        env.push(("RUST_LOG".into(), RUST_LOG.into()));
        let term = Term::spawn(TermConfig {
            program: PathBuf::from(env!("CARGO_BIN_EXE_loomcoding")),
            args: vec![],
            cwd: ws.project.path().to_path_buf(),
            env,
            rows: 40,
            cols: 120,
        })
        .unwrap();
        Session { term, ws, provider }
    }

    /// Espera a que aparezca `needle` en la pantalla (la TUI lista para teclas, por ejemplo).
    fn expect(&self, needle: &str, within: Duration) -> Snapshot {
        self.term
            .wait_for_text(needle, within)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Espera a que la pantalla cumpla `pred`. Un diálogo de permiso (título «Permiso») se concede con «s»
    /// solo si `concede` lo acepta.
    fn espera(
        &mut self,
        what: &str,
        pred: impl Fn(&str) -> bool,
        concede: impl Fn(&str) -> bool,
    ) -> Snapshot {
        let started = Instant::now();
        let mut concedido = false;
        loop {
            let snap = self.term.snapshot();
            if pred(&snap.text) {
                return snap;
            }
            let dialogo = snap.text.contains("Permiso") && snap.text.contains("Permitir");
            if !dialogo {
                concedido = false;
            }
            if dialogo && !concedido && concede(&snap.text) {
                self.term.send_text("s").unwrap();
                concedido = true;
            }
            if started.elapsed() > TURN {
                panic!("no apareció {what}; pantalla:\n{}", snap.text);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Contenidos de los mensajes `tool` que el proveedor ha recibido, en el orden de llegada.
    async fn resultados_de_herramienta(&self) -> Vec<String> {
        let mut resultados = Vec::new();
        for peticion in self.provider.received_requests().await.unwrap_or_default() {
            let cuerpo: serde_json::Value =
                serde_json::from_slice(&peticion.body).unwrap_or_default();
            for mensaje in cuerpo["messages"].as_array().into_iter().flatten() {
                if mensaje["role"] == "tool" {
                    if let Some(texto) = mensaje["content"].as_str() {
                        resultados.push(texto.to_string());
                    }
                }
            }
        }
        resultados
    }

    /// Línea del registro del daemon que emite el destino `output_filter`, esperándola un rato si hace falta.
    fn linea_de_registro(&self) -> Option<String> {
        let ruta = self
            .ws
            .home
            .path()
            .join(".loomcoding")
            .join("logs")
            .join("loomcoding-daemon.log");
        let started = Instant::now();
        loop {
            let texto = std::fs::read_to_string(&ruta).unwrap_or_default();
            if let Some(linea) = texto.lines().find(|linea| linea.contains(DESTINO)) {
                return Some(linea.to_string());
            }
            if started.elapsed() > TURN {
                return None;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn shot(&self, name: &str) {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("e2e-shots");
        self.term.save(&dir, name).unwrap();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        kill_daemons_of(self.ws.home.path());
    }
}

/// Ruta del original que anuncia la cabecera `[filtrado: … el original sigue en <ruta>]`.
fn ruta_del_original(cabecera: &str) -> PathBuf {
    let inicio = cabecera
        .find(MARCA_RUTA)
        .map(|i| i + MARCA_RUTA.len())
        .unwrap_or_else(|| panic!("la cabecera no anuncia el original: {cabecera}"));
    let resto = &cabecera[inicio..];
    let fin = resto.rfind(']').unwrap_or(resto.len());
    PathBuf::from(resto[..fin].trim())
}

/// Con el filtro encendido: el modelo recibe la cabecera, el original íntegro está en el volcado anunciado, el
/// registro tiene la traza con regla y bytes y el transcript muestra la cabecera.
#[tokio::test(flavor = "multi_thread")]
async fn filtered_output_reaches_the_model_with_header_original_and_trace() {
    // Como en el resto de E2E de la TUI: sin el muñeco braille de color.
    std::env::remove_var("NO_COLOR");
    let mut s = Session::start(true).await;
    s.expect(READY, BOOT);
    s.term.send_text(PETICION).unwrap();
    s.term.press(Key::Enter).unwrap();
    s.espera(
        "la respuesta final",
        |t| t.contains(COMPRESION_FINAL),
        |t| t.contains("bash"),
    );
    s.shot("compresion-1-turno");

    // 1. Lo que recibió el modelo llevaba la cabecera de filtrado con la regla general.
    let resultados = s.resultados_de_herramienta().await;
    let cabecera = resultados
        .iter()
        .find(|texto| texto.contains(CABECERA))
        .unwrap_or_else(|| panic!("el modelo no recibió cabecera de filtrado: {resultados:?}"));
    assert!(cabecera.contains("regla general"), "{cabecera}");

    // 2. El original íntegro (40 «ruido» y el «FIN») está en el volcado que la cabecera anuncia.
    let ruta = ruta_del_original(cabecera);
    let original = std::fs::read_to_string(&ruta)
        .unwrap_or_else(|error| panic!("no existe el original {}: {error}", ruta.display()));
    assert_eq!(
        original.lines().filter(|linea| *linea == "ruido").count(),
        40,
        "el original debe conservar las 40 líneas: {original}"
    );
    assert!(original.lines().any(|linea| linea == "FIN"), "{original}");

    // 3. El registro del daemon tiene la traza del destino output_filter, con regla y bytes.
    let linea = s
        .linea_de_registro()
        .unwrap_or_else(|| panic!("el registro no recogió ninguna línea de {DESTINO}"));
    assert!(linea.contains("regla=\"general\""), "{linea}");
    assert!(
        linea.contains("original_bytes=") && linea.contains("sent_bytes="),
        "{linea}"
    );

    // 4. El transcript muestra la cabecera de filtrado.
    let snap = s.term.snapshot();
    assert!(
        snap.text.contains(CABECERA),
        "el transcript debía mostrar la cabecera; pantalla:\n{}",
        snap.text
    );
    s.shot("compresion-2-cabecera");
}

/// Sin `[output_filter] enabled = true`, aunque `RUST_LOG` esté puesta: el modelo recibe la salida tal cual, sin
/// cabecera, y el registro no tiene ninguna línea del destino `output_filter`.
#[tokio::test(flavor = "multi_thread")]
async fn without_enabled_there_is_no_header_and_no_trace() {
    std::env::remove_var("NO_COLOR");
    let mut s = Session::start(false).await;
    s.expect(READY, BOOT);
    s.term.send_text(PETICION).unwrap();
    s.term.press(Key::Enter).unwrap();
    let snap = s.espera(
        "la respuesta final",
        |t| t.contains(COMPRESION_FINAL),
        |t| t.contains("bash"),
    );
    s.shot("compresion-3-apagado");

    let resultados = s.resultados_de_herramienta().await;
    assert!(
        resultados.iter().any(|texto| texto.contains("ruido")),
        "el modelo debía recibir la salida original: {resultados:?}"
    );
    assert!(
        !resultados.iter().any(|texto| texto.contains(CABECERA)),
        "sin enabled no debe haber cabecera: {resultados:?}"
    );
    assert!(
        !snap.text.contains(CABECERA),
        "sin enabled el transcript no debe mostrar cabecera; pantalla:\n{}",
        snap.text
    );
    let registro = s.ws.home.path().join(".loomcoding").join("logs");
    let texto = std::fs::read_dir(&registro)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entrada| std::fs::read_to_string(entrada.ok()?.path()).ok())
        .collect::<String>();
    assert!(
        !texto.contains(DESTINO),
        "sin enabled no debe haber líneas del destino {DESTINO}: {texto}"
    );
}

/// Comprobación de la ayuda de ruta: el marcador del original sale de la cabecera, no de un texto fijo.
#[test]
fn la_ruta_del_original_sale_de_la_cabecera() {
    let cabecera = "[filtrado: 39 líneas omitidas de 41 — regla general; el original sigue en C:\\work\\x\\orig.txt]";
    assert_eq!(
        ruta_del_original(cabecera),
        PathBuf::from("C:\\work\\x\\orig.txt")
    );
}
