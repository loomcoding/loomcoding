#![cfg(windows)]
//! E2E de la vigilancia de jobs en la TUI real: un `job_start` con `watch` imprime una línea que
//! casa con el patrón; el aviso `[Job … watch]` sale en el transcript con el actor «job» y el
//! modelo despierta para contestarlo. Como en `tui_pty.rs`, el perfil es temporal (`LOOMCODING_HOME`)
//! y el proveedor es un `wiremock` con el guion de `mock_provider_watch`: no hay red real.

use loomcoding_e2e::{
    kill_daemons_of, mock_provider_watch, Key, Snapshot, Term, TermConfig, Workspace,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wiremock::MockServer;

const BOOT: Duration = Duration::from_secs(40);
const TURN: Duration = Duration::from_secs(25);
/// Pista del composer vacío: indica que la TUI ya está lista para recibir teclas.
const READY: &str = "Describe what you want to build";
/// Respuesta del modelo cuando le llega el aviso de vigilancia (el guion la distingue de la del lanzamiento).
const DESPERTADO: &str = "aviso recibido";

struct Session {
    term: Term,
    ws: Workspace,
    _provider: MockServer,
}

impl Session {
    async fn start(rows: u16, cols: u16) -> Session {
        let provider = mock_provider_watch().await;
        let ws = Workspace::new(&provider.uri()).unwrap();
        let term = Term::spawn(TermConfig {
            program: PathBuf::from(env!("CARGO_BIN_EXE_loomcoding")),
            args: vec![],
            cwd: ws.project.path().to_path_buf(),
            env: ws.env(),
            rows,
            cols,
        })
        .unwrap();
        Session {
            term,
            ws,
            _provider: provider,
        }
    }

    fn expect(&self, needle: &str, within: Duration) -> Snapshot {
        self.term
            .wait_for_text(needle, within)
            .unwrap_or_else(|error| panic!("{error}"))
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

/// Un job vigilado imprime «FAIL uno»: el aviso llega al transcript como línea de «job» y el modelo
/// despierta y contesta con el guion de despertar. Las peticiones de permiso de `job_start` se
/// conceden en la sesión («s»), como en `tui_pty.rs`.
#[tokio::test(flavor = "multi_thread")]
async fn a_watched_job_notice_reaches_the_transcript_and_wakes_the_model() {
    // El muñeco braille se apaga con `NO_COLOR`, como en el resto de E2E de la TUI.
    std::env::remove_var("NO_COLOR");
    let mut s = Session::start(40, 120).await;
    s.expect(READY, BOOT);
    s.term.send_text("vigila los fallos del build").unwrap();
    s.term.press(Key::Enter).unwrap();

    let started = Instant::now();
    loop {
        let snap = s.term.snapshot();
        if snap.text.contains(DESPERTADO) {
            break;
        }
        if snap.text.contains("Permitir") {
            s.term.send_text("s").unwrap();
        }
        if started.elapsed() > TURN {
            panic!(
                "el modelo no despertó con el aviso; pantalla:\n{}",
                snap.text
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    s.shot("job-watch-wake");

    // La nota de vigilancia del job (tarjeta y rail) recoge el aviso entregado; llega por el bus, así
    // que se espera a que la TUI la pinte.
    let started = Instant::now();
    let snap = loop {
        let snap = s.term.snapshot();
        if snap.text.contains("watching: 1 alert") {
            break snap;
        }
        if started.elapsed() > TURN {
            panic!(
                "la nota de vigilancia no pasó a «watching: 1 alert»; pantalla:\n{}",
                snap.text
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    s.shot("job-watch-count");

    // El aviso sale en el transcript como línea del actor «job» (prefijo `job: `) con su cabecera
    // «[Job <id> watch]» y la línea que casa. No basta con «job» y «FAIL uno» sueltos: la llamada
    // `job_start` también los tiene, y el prefijo `sys: ` ya no debe aparecer en esta línea.
    assert!(
        snap.text
            .lines()
            .any(|l| l.contains("job: [Job ") && l.contains("watch] FAIL uno")),
        "falta el aviso de vigilancia con su cabecera y su línea; pantalla:\n{}",
        snap.text
    );
}
