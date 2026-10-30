#![cfg(windows)]
//! E2E de la TUI real en un pseudo-terminal: se lanza el binario, se le envían teclas y se
//! lee la pantalla interpretada. El proveedor es un `wiremock` y el perfil de usuario es
//! temporal, así que no se toca ni la sesión ni el daemon de la persona usuaria.

use codeloom_e2e::{
    kill_daemons_of, mock_provider, mock_provider_empty_subagent, Key, Snapshot, Term, TermConfig,
    Workspace,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wiremock::MockServer;

const BOOT: Duration = Duration::from_secs(40);
const TURN: Duration = Duration::from_secs(25);
/// Pista del composer vacío: indica que la TUI ya está lista para recibir teclas.
const READY: &str = "Describe what you want to build";

struct Session {
    term: Term,
    ws: Workspace,
    _provider: MockServer,
}

impl Session {
    async fn start(reply: &str, rows: u16, cols: u16) -> Session {
        Session::with_provider(mock_provider(reply).await, rows, cols)
    }

    fn with_provider(provider: MockServer, rows: u16, cols: u16) -> Session {
        let ws = Workspace::new(&provider.uri()).unwrap();
        let term = Term::spawn(TermConfig {
            program: PathBuf::from(env!("CARGO_BIN_EXE_codeloom")),
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

    /// Espera a que alguna línea de la pantalla cumpla `pred`.
    fn expect_line(&self, what: &str, pred: impl Fn(&str) -> bool) -> Snapshot {
        let started = Instant::now();
        loop {
            let snap = self.term.snapshot();
            if snap.text.lines().any(&pred) {
                return snap;
            }
            if started.elapsed() > TURN {
                panic!("no apareció {what}; pantalla:\n{}", snap.text);
            }
            std::thread::sleep(Duration::from_millis(50));
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

#[tokio::test(flavor = "multi_thread")]
async fn starts_and_shows_header_and_composer() {
    let s = Session::start("ok", 40, 120).await;
    let snap = s.expect(READY, BOOT);
    s.shot("start");
    assert!(
        s.ws.home
            .path()
            .join(".codeloom")
            .join("sessions.db")
            .is_file(),
        "el binario no usó el perfil temporal: se habría hablado con el daemon real"
    );
    assert!(
        snap.cursor.0 >= snap.rows * 2 / 3,
        "el cursor debía estar en el composer (tercio inferior): {:?}\n{}",
        snap.cursor,
        snap.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_shows_prompt_as_you_and_streamed_reply() {
    let mut s = Session::start("respuesta simulada", 40, 120).await;
    s.expect(READY, BOOT);
    s.term.send_text("hola").unwrap();
    s.term.press(Key::Enter).unwrap();
    let snap = s.expect("respuesta simulada", TURN);
    s.shot("turn");
    assert!(
        snap.text.contains("You"),
        "falta la atribución You:\n{}",
        snap.text
    );
    assert!(
        snap.text.contains("hola"),
        "falta el prompt:\n{}",
        snap.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn large_multiline_paste_keeps_composer_alive() {
    let mut s = Session::start("ok", 40, 120).await;
    s.expect(READY, BOOT);
    let block: String = (1..=200).map(|n| format!("linea {n}\n")).collect();
    s.term.paste(&block).unwrap();
    s.expect("[Pasted Content", TURN);
    s.term.send_text("x").unwrap();
    s.expect_line("el marcador seguido de lo escrito", |l| {
        l.contains("[Pasted Content") && l.contains("chars]x")
    });
    s.shot("paste");
}

#[tokio::test(flavor = "multi_thread")]
async fn tiny_terminal_does_not_panic() {
    let s = Session::start("ok", 20, 60).await;
    s.expect("CODEL", BOOT);
    s.shot("tiny");
    assert_eq!(
        s.term.exit_code(),
        None,
        "el hijo murió en una terminal pequeña"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn huge_crlf_paste_is_bounded() {
    let mut s = Session::start("ok", 40, 120).await;
    s.expect(READY, BOOT);
    let block: String = (1..=5000).map(|n| format!("linea {n}\r\n")).collect();
    s.term.paste(&block).unwrap();
    s.expect("[Pasted Content", TURN);
    s.term.send_text("y").unwrap();
    s.expect_line("el marcador seguido de lo escrito", |l| {
        l.contains("[Pasted Content") && l.contains("chars]y")
    });
    assert_eq!(s.term.exit_code(), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn no_orphan_daemon_after_session() {
    let s = Session::start("ok", 40, 120).await;
    s.expect(READY, BOOT);
    let home = s.ws.home.path().to_path_buf();
    assert!(
        kill_daemons_of(&home) >= 1,
        "debía haber un daemon `--server` del perfil temporal"
    );
    drop(s);
    assert_eq!(kill_daemons_of(&home), 0, "quedó un daemon huérfano");
}

#[tokio::test(flavor = "multi_thread")]
async fn ctrl_j_and_paste_add_lines_and_only_enter_sends() {
    let mut s = Session::start("respuesta del bloque", 40, 120).await;
    s.expect(READY, BOOT);
    s.term.send_text("uno").unwrap();
    s.term.press(Key::CtrlJ).unwrap();
    s.term.send_text("dos").unwrap();
    s.term
        .paste(
            "tres
cuatro
",
        )
        .unwrap();
    // El composer enseña como mucho tres filas: `uno` queda desplazado hacia arriba.
    let before = s.expect("cuatro", TURN);
    s.shot("multiline");
    assert!(
        before.text.lines().any(|l| l.contains("dostres")),
        "el pegado debía continuar la línea de `dos`:
{}",
        before.text
    );
    assert!(
        !before.text.contains("respuesta del bloque"),
        "un salto de línea envió el prompt:
{}",
        before.text
    );
    s.term.press(Key::Enter).unwrap();
    let after = s.expect("respuesta del bloque", TURN);
    // Ctrl+J separó `uno` de `dos`: son líneas distintas del prompt enviado.
    assert!(
        after
            .text
            .lines()
            .any(|l| l.contains("uno") && !l.contains("dos")),
        "falta `uno` en su propia línea:
{}",
        after.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn subagent_without_final_text_fails_instead_of_completing_empty() {
    let provider = mock_provider_empty_subagent("delegado, espero al hijo").await;
    let mut s = Session::with_provider(provider, 40, 120);
    s.expect(READY, BOOT);
    s.term.send_text("delega la inspeccion").unwrap();
    s.term.press(Key::Enter).unwrap();
    // Tanto `subagent` como el `ls` del hijo piden permiso; `s` es «Permitir en la sesión».
    let started = Instant::now();
    let snap = loop {
        let snap = s.term.snapshot();
        if snap.text.contains("step limit") {
            break snap;
        }
        if snap.text.contains("Permitir") {
            s.term.send_text("s").unwrap();
        }
        if started.elapsed() > TURN {
            panic!(
                "no apareció el fallo del hijo; pantalla:
{}",
                snap.text
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    s.shot("subagent-empty");
    // El texto se parte en dos filas, de ahí que se compruebe solo el principio del motivo.
    assert!(
        snap.text.contains("(3/3) without a final"),
        "falta el motivo del fallo con el límite:
{}",
        snap.text
    );
    assert!(
        snap.text
            .lines()
            .any(|l| l.contains("inspector") && l.contains("(failed)")),
        "el hijo debía figurar como failed:
{}",
        snap.text
    );
    assert!(
        !snap.text.contains("(completed)"),
        "un hijo sin respuesta no puede figurar como completado:
{}",
        snap.text
    );
}
