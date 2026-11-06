#![cfg(windows)]
//! E2E del carril del transcript en la TUI real, en un pseudo-terminal. Para cada valor de
//! `rail` se siembra `ui.toml` con `layout = "minimal"` (sin paneles laterales, cuyos marcos redondeados falsearían las aserciones de glifos), se envía un mensaje de dos líneas y se
//! comprueba en pantalla que la TUI sigue viva, que el texto aparece y que los glifos del carril
//! son los esperados. El perfil es temporal (`Workspace`) y el proveedor es un `wiremock`.

use loomcoding_e2e::{kill_daemons_of, mock_provider, Key, Snapshot, Term, TermConfig, Workspace};
use std::path::PathBuf;
use std::time::Duration;
use wiremock::MockServer;

const BOOT: Duration = Duration::from_secs(40);
const TURN: Duration = Duration::from_secs(25);
/// Pista del composer vacío: indica que la TUI ya está lista para recibir teclas.
const READY: &str = "Describe what you want to build";
/// Respuesta del proveedor simulado; sirve de marca para saber que el turno terminó.
const RESPUESTA: &str = "respuesta del carril";
const ROWS: u16 = 40;
const COLS: u16 = 120;

/// Sesión mínima: el terminal, el perfil temporal y el proveedor (que sigue vivo mientras dure la
/// prueba, de ahí que se guarde aunque no se lea).
struct Session {
    term: Term,
    ws: Workspace,
    _provider: MockServer,
}

impl Session {
    /// Lanza la TUI con el `ui.toml` que escribe `prepare` ya en el perfil.
    fn spawn(provider: MockServer, prepare: impl FnOnce(&Workspace)) -> Session {
        let ws = Workspace::new(&provider.uri()).unwrap();
        prepare(&ws);
        // Cabe señalar que el muñeco braille se apaga con `NO_COLOR`; se retira como en los demás E2E.
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

/// Arranca la TUI con `layout = "minimal"` (sin paneles laterales, cuyos marcos redondeados falsearían las aserciones de glifos) y `rail = "<rail>"`, envía un mensaje de dos líneas
/// (`uno`, Ctrl+J, `dos`, Enter) y espera la respuesta del proveedor simulado. Guarda el volcado
/// `carril-<rail>` y devuelve la sesión (para comprobar que sigue viva) junto a la pantalla final.
async fn turno_con_carril(rail: &str) -> (Session, Snapshot) {
    let provider = mock_provider(RESPUESTA).await;
    let contenido = format!("layout = \"minimal\"\nrail = \"{rail}\"\n");
    let mut s = Session::spawn(provider, |ws| {
        std::fs::write(
            ws.home.path().join(".loomcoding").join("ui.toml"),
            &contenido,
        )
        .unwrap();
    });
    s.expect(READY, BOOT);
    s.term.send_text("uno").unwrap();
    s.term.press(Key::CtrlJ).unwrap();
    s.term.send_text("dos").unwrap();
    s.term.press(Key::Enter).unwrap();
    s.expect(RESPUESTA, TURN);
    // Se captura en reposo: durante el turno la animación de fondo ensucia el volcado.
    s.expect("mock-model · ready", TURN);
    let snap = s.term.snapshot();
    s.shot(&format!("carril-{rail}"));
    assert_eq!(
        s.term.exit_code(),
        None,
        "la TUI murió con el carril `{rail}`:\n{}",
        snap.text
    );
    for texto in ["uno", "dos", RESPUESTA] {
        assert!(
            snap.text.contains(texto),
            "falta `{texto}` con el carril `{rail}`:\n{}",
            snap.text
        );
    }
    (s, snap)
}

#[tokio::test(flavor = "multi_thread")]
async fn carril_tree_dibuja_apertura_pero_no_cierre_de_caja() {
    let (_s, snap) = turno_con_carril("tree").await;
    assert!(
        snap.text.contains('╭'),
        "el carril `tree` no dibuja `╭`:\n{}",
        snap.text
    );
    assert!(
        !snap.text.contains('╮'),
        "el carril `tree` dibuja `╮`:\n{}",
        snap.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn carril_box_encierra_el_mensaje_del_usuario() {
    let (_s, snap) = turno_con_carril("box").await;
    for esquina in ['╭', '╰', '╮', '╯'] {
        assert!(
            snap.text.contains(esquina),
            "el carril `box` no dibuja `{esquina}`:\n{}",
            snap.text
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn carril_dots_sigue_viva_con_su_texto() {
    // Sin aserción de glifos: la comprobación común de `turno_con_carril` basta.
    let _ = turno_con_carril("dots").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn carril_bar_no_dibuja_caja() {
    let (_s, snap) = turno_con_carril("bar").await;
    assert!(
        !snap.text.contains('╭'),
        "el carril `bar` dibuja `╭`:\n{}",
        snap.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn carril_tint_no_dibuja_caja() {
    let (_s, snap) = turno_con_carril("tint").await;
    assert!(
        !snap.text.contains('╭'),
        "el carril `tint` dibuja `╭`:\n{}",
        snap.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn carril_none_no_dibuja_caja() {
    let (_s, snap) = turno_con_carril("none").await;
    assert!(
        !snap.text.contains('╭'),
        "el carril `none` dibuja `╭`:\n{}",
        snap.text
    );
}
