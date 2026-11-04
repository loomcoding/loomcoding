//! Utilidades: entradas acotadas, rutas de texto, huella, prompts y lanzamiento de hijos.

use super::*;
use crate::gauntlet::{AbOrder, GauntletProblem};
use sha2::{Digest, Sha256};

/// Tope de tamaño de un fichero de la pieza o de la barra: son texto que el crítico lee entero.
const MAX_TEXT_FILE_BYTES: u64 = 1_000_000;

pub(super) fn problem(error: GauntletProblem) -> CoreError {
    CoreError::Tool(error.to_string())
}

pub(super) fn bounded(input: &serde_json::Value, key: &str, max: usize) -> Result<String> {
    input[key]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= max)
        .map(|s| s.trim().to_owned())
        .ok_or_else(|| CoreError::Tool(format!("{key} must be 1 to {max} bytes")))
}

/// Resuelve `relative` dentro del proyecto y devuelve sus bytes si es un fichero de texto UTF-8
/// de tamaño acotado. Una ruta ausente, ajena al proyecto o no textual es un error.
fn read_text_file(cwd: &std::path::Path, relative: &str) -> Result<Vec<u8>> {
    let root = cwd
        .canonicalize()
        .map_err(|e| CoreError::Tool(e.to_string()))?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| CoreError::Tool(format!("cannot read `{relative}`: {e}")))?;
    if !path.starts_with(&root) {
        return Err(CoreError::Tool(format!(
            "`{relative}` is outside the project"
        )));
    }
    let metadata = std::fs::metadata(&path).map_err(|e| CoreError::Tool(e.to_string()))?;
    if !metadata.is_file() || metadata.len() > MAX_TEXT_FILE_BYTES {
        return Err(CoreError::Tool(format!(
            "`{relative}` must be a text file of at most {MAX_TEXT_FILE_BYTES} bytes"
        )));
    }
    let bytes = std::fs::read(&path).map_err(|e| CoreError::Tool(e.to_string()))?;
    if std::str::from_utf8(&bytes).is_err() {
        return Err(CoreError::Tool(format!("`{relative}` is not UTF-8 text")));
    }
    Ok(bytes)
}

pub(super) fn check_text_path(cwd: &std::path::Path, relative: &str) -> Result<()> {
    read_text_file(cwd, relative).map(|_| ())
}

/// Huella del artefacto: liga la aceptación de la persona a lo que se le presentó.
pub(super) fn artifact_hash(cwd: &std::path::Path, relative: &str) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(read_text_file(cwd, relative)?)
    ))
}

/// Semilla del sorteo A/B. No necesita ser criptográfica, solo no repetirse de forma sistemática.
pub(super) fn draw_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()) ^ d.as_secs())
        .unwrap_or(0);
    nanos ^ (nanos >> 7)
}

/// Primera ronda: el objetivo, la pieza y la barra a la que apuntar.
pub(super) fn first_builder_prompt(gauntlet: &Gauntlet) -> String {
    format!(
        "Improve the piece below until it reaches the quality of the quality bar. Edit it in place; do not modify the bar files.\nGoal: {}\nPiece to improve: {}\nQuality bar (read-only files to aim at): {}\nReport briefly what you changed and why.",
        gauntlet.goal,
        gauntlet.artifact,
        gauntlet.bar.join(", ")
    )
}

/// Rondas siguientes: solo la brecha del crítico, nunca su informe entero.
pub(super) fn rebuild_prompt(gauntlet: &Gauntlet, gap: &str) -> String {
    format!(
        "Your previous version was compared with the quality bar and the most important remaining gap is:\n{gap}\nFix this gap by editing the piece in place.\nGoal: {}\nPiece: {}\nReport briefly what you changed and why.",
        gauntlet.goal, gauntlet.artifact
    )
}

/// Prompt del crítico: dos candidatos sin rótulo de cuál es la barra y sin informe del constructor.
pub(super) fn critic_prompt(gauntlet: &Gauntlet, order: AbOrder) -> String {
    let artifact = std::slice::from_ref(&gauntlet.artifact);
    let (a, b) = match order {
        AbOrder::ArtifactFirst => (artifact, gauntlet.bar.as_slice()),
        AbOrder::BarFirst => (gauntlet.bar.as_slice(), artifact),
    };
    let list = |paths: &[String]| {
        paths
            .iter()
            .map(|path| format!("- {path}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "You are an independent critic with a clean context and read-only access. Two candidates are shown, A and B; you are not told which is which. Inspect the real files yourself and judge only what they contain; do not trust any description of them.\nGoal: {}\n\nCandidate A:\n{}\n\nCandidate B:\n{}\n\nDecide which candidate better meets the goal, or say they are equivalent. End your report with exactly one JSON object on its own last line: {{\"preferred\":\"A\" or \"B\" or \"tie\",\"biggest_gap\":\"one short sentence: the most important thing the less preferred candidate lacks\"}}. Always include biggest_gap unless preferred is tie.",
        gauntlet.goal,
        list(a),
        list(b)
    )
}

/// Guarda el cambio de fase y lo publica con su actor y su detalle: es el único camino por el que el
/// gauntlet cambia, así que nada queda sin verse en la conversación.
pub(super) async fn save(
    bus: &crate::event::EventBus,
    store: &dyn crate::session::SessionStore,
    gauntlet: Gauntlet,
    actor: &str,
    detail: &str,
) -> Result<Gauntlet> {
    let saved = store.update_gauntlet(gauntlet, actor, detail).await?;
    bus.publish(crate::event::LiveEvent::GauntletChanged {
        summary: saved.summary(),
        actor: actor.to_owned(),
        detail: detail.to_owned(),
    });
    Ok(saved)
}

/// Guarda el cambio de fase y, si falla, deja el gauntlet fallido en vez de colgado.
async fn launch_failed(
    bus: &crate::event::EventBus,
    store: &dyn crate::session::SessionStore,
    mut gauntlet: Gauntlet,
    error: CoreError,
) -> CoreError {
    if gauntlet.fail().is_ok() {
        gauntlet.active_child = None;
        let _ = save(
            bus,
            store,
            gauntlet,
            "coordinator",
            &format!("launch failed: {error}"),
        )
        .await;
    }
    error
}

async fn attach(
    bus: &crate::event::EventBus,
    store: &dyn crate::session::SessionStore,
    mut gauntlet: Gauntlet,
    child: crate::session::SessionId,
    detail: &str,
) -> Result<Gauntlet> {
    gauntlet.attach_child(child);
    save(bus, store, gauntlet, "coordinator", detail).await
}

/// Lanza al constructor (con escritura) y registra el hijo.
pub(super) async fn launch_builder(
    bus: &crate::event::EventBus,
    store: &dyn crate::session::SessionStore,
    launcher: &dyn crate::subagent::SubagentLauncher,
    gauntlet: Gauntlet,
    prompt: String,
    cancel: &CancellationToken,
) -> Result<Gauntlet> {
    match launcher
        .start_local_workflow_agent(
            gauntlet.parent,
            "gauntlet builder".into(),
            prompt,
            gauntlet.builder.clone(),
            cancel.clone(),
        )
        .await
    {
        Ok((child, _)) => attach(bus, store, gauntlet, child, "builder launched").await,
        Err(error) => Err(launch_failed(bus, store, gauntlet, error).await),
    }
}

/// Lanza un crítico nuevo, de solo lectura, y registra el hijo.
pub(super) async fn launch_critic(
    bus: &crate::event::EventBus,
    store: &dyn crate::session::SessionStore,
    launcher: &dyn crate::subagent::SubagentLauncher,
    gauntlet: Gauntlet,
    order: AbOrder,
    cancel: &CancellationToken,
) -> Result<Gauntlet> {
    match launcher
        .start_read_only_review(
            gauntlet.parent,
            "gauntlet critic".into(),
            critic_prompt(&gauntlet, order),
            gauntlet.critic.clone(),
            cancel.clone(),
        )
        .await
    {
        Ok((child, _)) => attach(bus, store, gauntlet, child, "critic launched").await,
        Err(error) => Err(launch_failed(bus, store, gauntlet, error).await),
    }
}

/// Cierra el gauntlet como fallido con el motivo y avisa al padre de que no continúe por su cuenta.
pub(super) async fn fail_gauntlet(
    bus: &crate::event::EventBus,
    store: &dyn crate::session::SessionStore,
    mut gauntlet: Gauntlet,
    reason: &str,
) -> Result<ToolOutput> {
    gauntlet.fail().map_err(problem)?;
    gauntlet.active_child = None;
    save(bus, store, gauntlet, "coordinator", reason).await?;
    Ok(ToolOutput::err(format!(
        "Gauntlet failed: {reason}. Do not edit the artifact as the parent; report this blocker to the user."
    )))
}
