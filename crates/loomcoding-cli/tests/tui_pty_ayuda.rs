#![cfg(windows)]
//! E2E de `/help` en la TUI real, en un pseudo-terminal: `/help model` abre la ayuda del comando
//! y Tab pasa al índice de temas. Como en `tui_pty_comandos.rs`, el binario corre con un perfil
//! temporal (`LOOMCODING_HOME`) y el proveedor es un `wiremock`: no hay red real ni se toca el
//! daemon de la persona usuaria.

use loomcoding_e2e::{kill_daemons_of, mock_provider, Key, Snapshot, Term, TermConfig, Workspace};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wiremock::MockServer;

const BOOT: Duration = Duration::from_secs(40);
const TURN: Duration = Duration::from_secs(25);
/// Pista del composer vacío: indica que la TUI ya está lista para recibir teclas.
const READY: &str = "Describe what you want to build";
const ROWS: u16 = 40;
const COLS: u16 = 130;

/// Sesión mínima: el terminal, el perfil temporal y el proveedor por defecto (que sigue vivo
/// mientras dure la prueba, de ahí que se guarde aunque no se lea).
struct Session {
    term: Term,
    ws: Workspace,
    _provider: MockServer,
}

impl Session {
    async fn start() -> Session {
        let provider = mock_provider("ok").await;
        let ws = Workspace::new(&provider.uri()).unwrap();
        // Como en el resto de e2e de la TUI: sin `NO_COLOR`, para que el muñeco se pinte igual.
        std::env::remove_var("NO_COLOR");
        let term = Term::spawn(TermConfig {
            program: PathBuf::from(env!("CARGO_BIN_EXE_loomcoding")),
            args: vec![],
            cwd: ws.project.path().to_path_buf(),
            env: ws.env(),
            rows: ROWS,
            cols: COLS,
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

    /// Espera a que el texto de la pantalla cumpla `pred`; con el tiempo agotado, falla con la pantalla.
    fn wait_until(&self, what: &str, pred: impl Fn(&str) -> bool) -> Snapshot {
        let started = Instant::now();
        loop {
            let snap = self.term.snapshot();
            if pred(&snap.text) {
                return snap;
            }
            if started.elapsed() > TURN {
                panic!("{what}; pantalla:\n{}", snap.text);
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
async fn help_shows_one_command_and_tab_opens_the_topics() {
    let mut s = Session::start().await;
    s.expect(READY, BOOT);

    s.term.send_text("/help model").unwrap();
    s.term.press(Key::Enter).unwrap();
    let snap = s.wait_until("no se dibujó la ayuda de /model", |t| {
        t.contains("Usage: /model [<id>]") && t.contains("Example: /model <id>")
    });
    s.shot("help-model");
    assert!(
        snap.text.contains("docs/user/en/reference/commands.md"),
        "falta el enlace a la referencia:\n{}",
        snap.text
    );

    // Tab cambia a la vista de temas: su pie y su lista son distintos de los de la ayuda de comando.
    s.term.press(Key::Tab).unwrap();
    let snap = s.wait_until("Tab no abrió el índice de temas", |t| {
        t.contains("Help topics") && t.contains("flows")
    });
    s.shot("help-topics");
    assert!(
        snap.text.contains("F1/Esc close · Tab shortcuts"),
        "pie del índice de temas:\n{}",
        snap.text
    );

    s.term.press(Key::Esc).unwrap();
    s.wait_until("la ayuda no se cerró con Esc", |t| {
        !t.contains("Help topics")
    });
    assert_eq!(
        s.term.exit_code(),
        None,
        "la TUI murió tras cerrar la ayuda"
    );
    s.expect(READY, TURN);
}
