#![cfg(windows)]
//! E2E de ODD (desarrollo orgánico) en la TUI real: la petición llega a `odd_track`, la cabecera
//! muestra la etiqueta ODD con su avance, el primer intento de cierre queda bloqueado por el gate y el
//! turno solo cierra cuando las dos tareas tienen evidencia de un `bash` de esta sesión. Con
//! `[flows.odd] enabled = false` el mismo guion no muestra etiqueta y el cierre no se bloquea.
//!
//! Como en `tui_pty_vigilancia.rs`, el perfil es temporal (`LOOMCODING_HOME`), el proyecto es temporal
//! y el proveedor es un `wiremock` con el guion de `mock_provider_odd`: no hay red real.

use loomcoding_e2e::{
    kill_daemons_of, mock_provider_odd, Key, Snapshot, Term, TermConfig, Workspace,
    ODD_CIERRE_FINAL, ODD_ID_BASH, ODD_INTENTO_DE_CIERRE,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wiremock::{MockServer, Request};

const BOOT: Duration = Duration::from_secs(40);
const TURN: Duration = Duration::from_secs(25);
/// Pista del composer vacío: indica que la TUI ya está lista para recibir teclas.
const READY: &str = "Describe what you want to build";
const PETICION: &str = "planifica la feature demo";
/// Prefijo del mensaje que la política de cierre devuelve al modelo.
const REINYECCION: &str = "[Desarrollo org";
/// Inicio del aviso que el gate publica cuando bloquea el cierre.
const AVISO_BLOQUEO: &str = "Tareas pendientes:";
/// Margen tras la respuesta final para que un cierre bloqueado (aviso y petición nueva) ya se vea.
const GRACIA: Duration = Duration::from_millis(1500);

struct Session {
    term: Term,
    ws: Workspace,
    provider: MockServer,
}

impl Session {
    /// Arranca la TUI con el guion de ODD. Con `odd_enabled` se escribe `[flows.odd] enabled` en el proyecto.
    async fn start(odd_enabled: Option<bool>) -> Session {
        let provider = mock_provider_odd().await;
        let ws = Workspace::new(&provider.uri()).unwrap();
        if let Some(enabled) = odd_enabled {
            let ruta = ws.project.path().join("loomcoding.toml");
            let mut texto = std::fs::read_to_string(&ruta).unwrap();
            texto.push_str(&format!("\n[flows.odd]\nenabled = {enabled}\n"));
            std::fs::write(&ruta, texto).unwrap();
        }
        let term = Term::spawn(TermConfig {
            program: PathBuf::from(env!("CARGO_BIN_EXE_loomcoding")),
            args: vec![],
            cwd: ws.project.path().to_path_buf(),
            env: ws.env(),
            rows: 40,
            cols: 120,
        })
        .unwrap();
        Session { term, ws, provider }
    }

    fn expect(&self, needle: &str, within: Duration) -> Snapshot {
        self.term
            .wait_for_text(needle, within)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Espera a que la pantalla cumpla `pred`. Un diálogo de permiso (título «Permiso») se concede
    /// con «s» (en la sesión) solo si `concede` lo acepta; los demás quedan abiertos y pausan el turno.
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

    /// Peticiones que el proveedor ha recibido cuyo cuerpo contiene `needle`.
    async fn pedidos_con(&self, needle: &str) -> usize {
        self.provider
            .received_requests()
            .await
            .unwrap_or_default()
            .iter()
            .filter(|peticion: &&Request| String::from_utf8_lossy(&peticion.body).contains(needle))
            .count()
    }

    /// Espera a que el proveedor reciba una petición que contenga `needle`.
    async fn espera_pedido(&self, needle: &str) {
        let started = Instant::now();
        while self.pedidos_con(needle).await == 0 {
            if started.elapsed() > TURN {
                panic!("el proveedor nunca recibió una petición con «{needle}»");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Espera la respuesta final y deja pasar `GRACIA` antes de mirar la pantalla. La TUI no pinta un
    /// indicador de fin de turno fiable (la fase «responding» no se reinicia al cerrar), así que el cierre
    /// se comprueba por ausencia: si el gate reinyectara, el aviso y una nueva petición llegarían en ese margen.
    fn espera_cierre(&mut self, respuesta_final: &str) -> Snapshot {
        self.espera(
            &format!("la respuesta final «{respuesta_final}»"),
            |t| t.contains(respuesta_final),
            |_| false,
        );
        std::thread::sleep(GRACIA);
        self.term.snapshot()
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

/// `true` si alguna línea de la pantalla contiene los dos fragmentos.
fn tiene_linea(texto: &str, a: &str, b: &str) -> bool {
    texto
        .lines()
        .any(|linea| linea.contains(a) && linea.contains(b))
}

/// Con ODD encendido: la etiqueta muestra el avance, el cierre prematuro se bloquea con su aviso y el
/// modelo lo recibe; tras el `bash` y las dos marcas, la etiqueta llega a (2/2) y el turno cierra.
#[tokio::test(flavor = "multi_thread")]
async fn odd_labels_the_header_and_blocks_the_close_until_both_tasks_have_evidence() {
    // El muñeco braille se apaga con `NO_COLOR`, como en el resto de E2E de la TUI.
    std::env::remove_var("NO_COLOR");
    let mut s = Session::start(None).await;
    s.expect(READY, BOOT);
    s.term.send_text(PETICION).unwrap();
    s.term.press(Key::Enter).unwrap();

    // 1. `odd_track` create: se concede su permiso; el de `bash` queda abierto y pausa el turno, así
    // que la etiqueta (0/2) se mantiene hasta que se conceda.
    s.espera(
        "la etiqueta ODD con avance (0/2) y las tareas T1 y T2",
        |t| tiene_linea(t, "ODD · ", "(0/2)") && t.contains("T1:") && t.contains("T2:"),
        |t| t.contains("odd_track"),
    );
    s.shot("odd-1-etiqueta");

    // 2. Cierre prematuro: el gate bloquea el cierre y reinyecta el mensaje al modelo.
    s.espera(
        "el aviso de cierre bloqueado",
        |t| t.contains(AVISO_BLOQUEO),
        |_| false,
    );
    s.shot("odd-2-bloqueo");
    s.espera_pedido(REINYECCION).await;

    // 3. El `bash` (con permiso) y las dos marcas con su evidencia: la etiqueta llega a (2/2).
    s.espera(
        "la etiqueta ODD con avance (2/2)",
        |t| tiene_linea(t, "ODD · ", "(2/2)"),
        |_| true,
    );
    s.shot("odd-3-avance");

    // 4. Con las dos tareas probadas el turno cierra: tras la respuesta final no hay ni otro aviso ni
    // otra reinyección. El aviso de bloqueo sigue en el transcript una sola vez.
    let snap = s.espera_cierre(ODD_CIERRE_FINAL);
    s.shot("odd-4-cierre");
    let avisos = snap
        .text
        .lines()
        .filter(|linea| linea.contains(AVISO_BLOQUEO))
        .count();
    assert_eq!(
        avisos, 1,
        "el cierre debía bloquearse una sola vez; pantalla:\n{}",
        snap.text
    );
    assert!(
        s.pedidos_con(&format!("\"tool_call_id\":\"{ODD_ID_BASH}\""))
            .await
            > 0,
        "el proveedor nunca recibió el resultado del bash de evidencia"
    );
}

/// Con `[flows.odd] enabled = false` el mismo guion no muestra la etiqueta ODD y el cierre no se bloquea.
#[tokio::test(flavor = "multi_thread")]
async fn odd_disabled_shows_no_label_and_does_not_block_the_close() {
    std::env::remove_var("NO_COLOR");
    let mut s = Session::start(Some(false)).await;
    s.expect(READY, BOOT);
    s.term.send_text(PETICION).unwrap();
    s.term.press(Key::Enter).unwrap();

    // Se crea la feature (con permiso) y el modelo intenta cerrar: el turno debe terminar ahí.
    s.espera(
        "el intento de cierre",
        |t| t.contains(ODD_INTENTO_DE_CIERRE),
        |t| t.contains("odd_track"),
    );
    let snap = s.espera_cierre(ODD_INTENTO_DE_CIERRE);
    assert!(
        !snap.text.contains("ODD · "),
        "con ODD apagado no debe haber etiqueta; pantalla:\n{}",
        snap.text
    );
    assert!(
        !snap.text.contains(AVISO_BLOQUEO),
        "con ODD apagado el cierre no debe bloquearse; pantalla:\n{}",
        snap.text
    );
    assert_eq!(
        s.pedidos_con(REINYECCION).await,
        0,
        "no debe haber reinyección"
    );
}
