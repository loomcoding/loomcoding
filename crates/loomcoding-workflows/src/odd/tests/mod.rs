//! Pruebas de `odd_track` con disco temporal y un `SqliteStore` real (en otro directorio temporal).

mod disco;
mod evidencia;
mod fase;
mod gate;

use crate::odd::FeatureDoc;
use loomcoding_core::event::EventBus;
use loomcoding_core::job::LocalJobRegistry;
use loomcoding_core::message::{ContentPart, Message};
use loomcoding_core::permission::PermissionService;
use loomcoding_core::session::sqlite::SqliteStore;
use loomcoding_core::session::{SessionId, SessionStore};
use loomcoding_core::tools::{ExecutionMode, Tool, ToolCtx, ToolOutput, ToolRegistry};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Entorno de una prueba: proyecto temporal, almacén temporal y una sesión.
pub(super) struct Entorno {
    pub proyecto: tempfile::TempDir,
    _db: tempfile::TempDir,
    pub store: Arc<SqliteStore>,
    pub sesion: SessionId,
    pub estado: crate::OddState,
    herramienta: Arc<dyn Tool>,
}

pub(super) async fn entorno() -> Entorno {
    let proyecto = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(db.path().join("s.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let sesion = store.create_session("proyecto").await.unwrap();
    let mut registro = ToolRegistry::new();
    let estado = crate::register(&mut registro);
    let herramienta = registro.get("odd_track").expect("odd_track registrada");
    Entorno {
        proyecto,
        _db: db,
        store,
        sesion,
        estado,
        herramienta,
    }
}

impl Entorno {
    /// Contexto de la herramienta. Con `con_store = false`, el almacén no está disponible.
    pub fn ctx(&self, con_store: bool) -> ToolCtx {
        ToolCtx {
            cwd: self.proyecto.path().to_path_buf(),
            store: con_store.then(|| self.store.clone() as Arc<dyn SessionStore>),
            permission: Arc::new(PermissionService::allow_all()),
            session: self.sesion,
            jobs: Arc::new(LocalJobRegistry::default()),
            bus: EventBus::new(8),
            subagents: None,
        }
    }

    pub async fn llamar(&self, input: Value) -> ToolOutput {
        self.llamar_con(self.ctx(true), input).await
    }

    pub async fn llamar_con(&self, ctx: ToolCtx, input: Value) -> ToolOutput {
        self.herramienta
            .run(ctx, input, CancellationToken::new())
            .await
            .unwrap()
    }

    pub fn ruta(&self, feature: &str) -> PathBuf {
        self.proyecto
            .path()
            .join(".loomcoding")
            .join("odd")
            .join(format!("{feature}.md"))
    }

    pub fn leer(&self, feature: &str) -> String {
        std::fs::read_to_string(self.ruta(feature)).unwrap()
    }

    /// Anota en el almacén una llamada de herramienta `id` (con nombre `nombre`) de la sesión dada.
    pub async fn registrar_llamada(&self, sesion: SessionId, id: &str, nombre: &str) {
        let msg = Message::assistant(vec![ContentPart::ToolCall {
            id: id.into(),
            name: nombre.into(),
            input: json!({}),
        }]);
        self.store.append_message(sesion, &msg).await.unwrap();
    }

    /// Crea una feature con tareas de texto simple `Tarea uno`, `Tarea dos`, ...
    pub async fn crear_basico(&self, feature: &str, tareas: &[&str]) {
        let out = self
            .llamar(json!({
                "op": "create",
                "feature": feature,
                "objective": "Objetivo de prueba",
                "tasks": tareas,
            }))
            .await;
        assert!(!out.is_error, "{}", out.content);
    }
}

pub(super) fn parsear(md: &str) -> FeatureDoc {
    FeatureDoc::parse(md).expect("el documento debe ser legible")
}

#[test]
fn register_anade_odd_track_en_modo_exclusivo() {
    let mut registro = ToolRegistry::new();
    let _estado = crate::register(&mut registro);
    let herramienta = registro.get("odd_track").expect("odd_track registrada");
    assert_eq!(herramienta.name(), "odd_track");
    assert_eq!(
        herramienta.execution_mode(&json!({"op": "read"})),
        ExecutionMode::Exclusive
    );
}

#[test]
fn register_devuelve_estados_que_comparten_memoria() {
    let mut registro = ToolRegistry::new();
    let estado = crate::register(&mut registro);
    let copia = estado.clone();
    let sesion = SessionId(7);
    copia.recordar(sesion, "alfa");
    assert_eq!(estado.active_feature(sesion).as_deref(), Some("alfa"));
}

#[tokio::test]
async fn una_operacion_desconocida_se_rechaza_con_la_lista() {
    let e = entorno().await;
    let out = e.llamar(json!({"op": "borrar", "feature": "f"})).await;
    assert!(out.is_error);
    assert!(out.content.contains("create"), "{}", out.content);
}

#[tokio::test]
async fn sin_campo_op_se_rechaza() {
    let e = entorno().await;
    let out = e.llamar(json!({"feature": "f"})).await;
    assert!(out.is_error);
    assert!(out.content.contains("op"), "{}", out.content);
}

#[tokio::test]
async fn nombres_de_feature_invalidos_se_rechazan_sin_escribir_fuera() {
    let nombres = [
        "..".to_string(),
        "../fuera".to_string(),
        "a/b".to_string(),
        "a\\b".to_string(),
        "Mayusculas".to_string(),
        String::new(),
        "x".repeat(65),
    ];
    for nombre in nombres {
        let e = entorno().await;
        let out = e
            .llamar(json!({"op": "create", "feature": nombre, "objective": "o", "tasks": ["t"]}))
            .await;
        assert!(out.is_error, "debía rechazar {nombre:?}");
        assert!(out.content.contains("no es válido"), "{}", out.content);
        assert!(
            !e.proyecto.path().join(".loomcoding").exists(),
            "no debe crearse nada para {nombre:?}"
        );
        let padre = e.proyecto.path().parent().unwrap();
        assert!(!padre.join("fuera.md").exists());
    }
}

#[tokio::test]
async fn un_nombre_de_64_caracteres_es_valido() {
    let e = entorno().await;
    let nombre = "x".repeat(64);
    let out = e
        .llamar(json!({"op": "create", "feature": nombre, "objective": "o", "tasks": ["t"]}))
        .await;
    assert!(!out.is_error, "{}", out.content);
}
