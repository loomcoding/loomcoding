#![cfg(windows)]
//! E2E de los comandos de la TUI real en un pseudo-terminal: `/ui`, `/effort`, `/model` y
//! `/provider`. Como en `tui_pty.rs`, el binario corre con un perfil temporal (`LOOMCODING_HOME`),
//! el proveedor por defecto es un `wiremock` y los proveedores locales son servidores de prueba
//! en `127.0.0.1`: no hay red real ni se toca el daemon de la persona usuaria.

use loomcoding_e2e::{kill_daemons_of, mock_provider, Key, Snapshot, Term, TermConfig, Workspace};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
    /// Lanza la TUI. `prepare` puede escribir ficheros en el perfil antes de que arranque.
    fn spawn(provider: MockServer, prepare: impl FnOnce(&Workspace)) -> Session {
        let ws = Workspace::new(&provider.uri()).unwrap();
        prepare(&ws);
        // Cabe señalar que el muñeco braille se apaga con `NO_COLOR`; se retira como en el test del menú.
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

    async fn start(reply: &str) -> Session {
        Session::spawn(mock_provider(reply).await, |_| {})
    }

    fn expect(&self, needle: &str, within: Duration) -> Snapshot {
        self.term
            .wait_for_text(needle, within)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Espera a que alguna línea de la pantalla cumpla `pred`.
    fn expect_line(&self, what: &str, pred: impl Fn(&str) -> bool) -> Snapshot {
        self.wait_until(what, |text| text.lines().any(&pred))
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

/// Servidor que contesta `GET /v1/models` con `status` y `body`, como un proveedor local.
async fn models_server(status: u16, body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

/// Puerto libre que ya nadie escucha: el sondeo de ese proveedor debe dar «down».
fn puerto_cerrado() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Siembra `models-dev.json` con un modelo de razonamiento para el proveedor por defecto, de modo
/// que `/effort` tenga niveles que ofrecer (con el catálogo vacío respondería que no hay niveles).
fn seed_reasoning_catalog(ws: &Workspace, base_url: &str) {
    let catalogo = json!({
        "mockprov": {
            "id": "mockprov",
            "name": "Mock",
            "npm": "@ai-sdk/openai-compatible",
            "api": base_url,
            "models": {
                "mock-model": {
                    "id": "mock-model",
                    "name": "Mock model",
                    "tool_call": true,
                    "reasoning": true,
                    "reasoning_options": [{"type": "effort", "values": ["low", "medium", "high"]}]
                }
            }
        }
    });
    std::fs::write(
        ws.home.path().join(".loomcoding").join("models-dev.json"),
        catalogo.to_string(),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn ui_panel_opens_and_old_aliases_are_hidden() {
    let mut s = Session::start("ok").await;
    s.expect(READY, BOOT);
    s.term.send_text("/ui").unwrap();
    s.term.press(Key::Enter).unwrap();
    // Filas del panel: «Distribución» solo aparece dentro de él.
    s.wait_until("no se dibujó el panel «Interfaz»", |t| {
        t.contains("Distribución") && t.contains("Interfaz")
    });
    s.shot("ui-panel");
    s.term.press(Key::Esc).unwrap();
    s.wait_until("el panel no se cerró con Esc", |t| {
        !t.contains("Distribución")
    });
    assert_eq!(s.term.exit_code(), None, "la TUI murió tras cerrar /ui");

    // Con el composer vacío, `/` despliega la lista de comandos.
    s.term.send_text("/").unwrap();
    let snap = s.expect("/ui", TURN);
    s.shot("slash-popup");
    for oculto in [
        "/theme", "/layout", "/crush", "/classic", "/mascot", "/sidebar", "/mouse", "/temas",
    ] {
        assert!(
            !snap.text.contains(oculto),
            "el popup lista el alias oculto `{oculto}`:\n{}",
            snap.text
        );
    }
    for visible in ["/ui", "/model", "/effort", "/provider"] {
        assert!(
            snap.text.contains(visible),
            "falta `{visible}` en el popup:\n{}",
            snap.text
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn effort_selector_shows_default_and_levels() {
    let provider = mock_provider("ok").await;
    let base = provider.uri();
    let mut s = Session::spawn(provider, |ws| seed_reasoning_catalog(ws, &base));
    s.expect(READY, BOOT);
    s.term.send_text("/effort").unwrap();
    s.term.press(Key::Enter).unwrap();
    let snap = s.expect("Enter apply · Esc cancel", TURN);
    s.shot("effort-picker");
    assert!(
        snap.text.lines().any(|l| l.contains("default")),
        "falta la fila «default»:\n{}",
        snap.text
    );
    for nivel in ["low", "medium", "high"] {
        assert!(
            snap.text.lines().any(|l| l.contains(nivel)),
            "falta el nivel `{nivel}`:\n{}",
            snap.text
        );
    }
    s.term.press(Key::Esc).unwrap();
    s.wait_until("el selector de esfuerzo no se cerró", |t| {
        !t.contains("Enter apply · Esc cancel")
    });
    assert_eq!(s.term.exit_code(), None, "la TUI murió tras cerrar /effort");
    s.expect(READY, TURN);
}

#[tokio::test(flavor = "multi_thread")]
async fn model_command_changes_the_footer() {
    let mut s = Session::start("ok").await;
    s.expect(READY, BOOT);
    s.term.send_text("/model prueba-modelo").unwrap();
    s.term.press(Key::Enter).unwrap();
    // Tras el Enter el composer se vacía: el modelo nuevo debe aparecer fuera del comando tecleado.
    let snap = s.wait_until("el modelo no aparece en pantalla", |t| {
        t.lines()
            .any(|l| l.contains("prueba-modelo") && !l.contains("/model"))
    });
    s.shot("model-changed");
    assert_eq!(
        s.term.exit_code(),
        None,
        "la TUI murió tras /model:\n{}",
        snap.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_selector_shows_discovered_local_servers() {
    let uno = models_server(
        200,
        json!({"data": [{"id": "modelo-a"}, {"id": "modelo-b"}]}),
    )
    .await;
    let dos = models_server(401, json!({"error": "falta la clave"})).await;
    let tres = format!("http://127.0.0.1:{}/v1", puerto_cerrado());
    let registro = format!(
        r#"default = "uno"

[[provider]]
id = "uno"
name = "Uno"
kind = "openai_compat"
base_url = "{}/v1"
model = "modelo-a"

[[provider]]
id = "dos"
name = "Dos"
kind = "openai_compat"
base_url = "{}/v1"
model = "modelo-b"

[[provider]]
id = "tres"
name = "Tres"
kind = "openai_compat"
base_url = "{tres}"
model = "modelo-c"
"#,
        uno.uri(),
        dos.uri(),
    );
    let mut s = Session::spawn(mock_provider("ok").await, |ws| {
        std::fs::write(
            ws.home.path().join(".loomcoding").join("providers.toml"),
            &registro,
        )
        .unwrap();
    });
    s.expect(READY, BOOT);
    // Cabe señalar que sin este margen `/provider` contesta «Providers are still loading»
    // (el catálogo con los locales llega después de READY). Los sondeos tardan como mucho 1,5 s.
    std::thread::sleep(Duration::from_secs(3));
    s.term.send_text("/provider").unwrap();
    s.term.press(Key::Enter).unwrap();
    s.expect_line("no apareció «Uno · up (2)»", |l| {
        l.contains("Uno · up (2)")
    });
    s.expect_line("no apareció «Dos · sin acceso»", |l| {
        l.contains("Dos · sin acceso")
    });
    let snap = s.expect_line("no apareció «Tres · down»", |l| {
        l.contains("Tres · down")
    });
    s.shot("provider-picker");
    assert!(
        snap.text.contains("Enter apply · Esc cancel"),
        "{}",
        snap.text
    );

    // Se baja con ↓ hasta que la fila resaltada (`›`) sea «Uno».
    let mut bajadas = 0;
    loop {
        let snap = s.term.snapshot();
        if snap
            .text
            .lines()
            .any(|l| l.contains('›') && l.contains("Uno · up (2)"))
        {
            break;
        }
        assert!(
            bajadas < 10,
            "no se llegó a «Uno» bajando; pantalla:\n{}",
            snap.text
        );
        s.term.press(Key::Down).unwrap();
        bajadas += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
    s.term.press(Key::Enter).unwrap();
    let snap = s.expect("modelo-a", TURN);
    s.shot("provider-selected");
    assert_eq!(
        s.term.exit_code(),
        None,
        "la TUI murió al elegir Uno:\n{}",
        snap.text
    );

    // El sondeo solo pregunta qué modelos hay: nada de inferencia ni rutas distintas.
    for (nombre, servidor) in [("uno", &uno), ("dos", &dos)] {
        let peticiones = servidor.received_requests().await.unwrap_or_default();
        assert!(
            !peticiones.is_empty(),
            "el proveedor {nombre} no recibió el sondeo"
        );
        for peticion in &peticiones {
            assert_eq!(
                (peticion.method.to_string().as_str(), peticion.url.path()),
                ("GET", "/v1/models"),
                "petición inesperada al proveedor {nombre}"
            );
        }
    }
}
