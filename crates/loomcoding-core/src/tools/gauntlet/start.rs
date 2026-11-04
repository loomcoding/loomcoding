//! Acción `start`: valida, crea el gauntlet y lanza al constructor.

use super::*;
use crate::gauntlet::{GauntletId, NewGauntlet};

const MAX_BAR_PATHS: usize = 8;

fn bar_paths(input: &serde_json::Value) -> Result<Vec<String>> {
    let paths = input["bar"]
        .as_array()
        .filter(|list| !list.is_empty() && list.len() <= MAX_BAR_PATHS)
        .ok_or_else(|| CoreError::Tool(format!("bar must list 1 to {MAX_BAR_PATHS} paths")))?;
    paths
        .iter()
        .map(|path| {
            path.as_str()
                .filter(|s| !s.trim().is_empty() && s.len() <= 512)
                .map(|s| s.trim().to_owned())
                .ok_or_else(|| CoreError::Tool("each bar path must be 1 to 512 bytes".into()))
        })
        .collect()
}

pub(super) async fn action_start(
    ctx: &ToolCtx,
    input: &serde_json::Value,
    cancel: &CancellationToken,
    store: &Store,
    launcher: &Launcher,
) -> Result<ToolOutput> {
    if store.active_gauntlet(ctx.session).await?.is_some() {
        return Err(CoreError::Tool("a gauntlet is already active".into()));
    }
    let goal = bounded(input, "goal", 8_000)?;
    let artifact = bounded(input, "artifact", 512)?;
    let builder = bounded(input, "builder", 64)?;
    let critic = bounded(input, "critic", 64)?;
    let bar = bar_paths(input)?;
    let max_rounds = match input.get("max_rounds").filter(|value| !value.is_null()) {
        None => None,
        Some(value) => Some(
            value
                .as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| CoreError::Tool("max_rounds must be an integer".into()))?,
        ),
    };
    check_text_path(&ctx.cwd, &artifact)?;
    for path in &bar {
        check_text_path(&ctx.cwd, path)?;
    }
    let gauntlet = Gauntlet::new(NewGauntlet {
        id: GauntletId(0),
        parent: ctx.session,
        goal,
        artifact,
        bar,
        builder,
        critic,
        max_rounds,
        now_ms: crate::message::now_ms(),
    })
    .map_err(problem)?;
    let gauntlet = store.create_gauntlet(gauntlet, "gauntlet started").await?;
    let prompt = first_builder_prompt(&gauntlet);
    let gauntlet =
        launch_builder(store.as_ref(), launcher.as_ref(), gauntlet, prompt, cancel).await?;
    Ok(ToolOutput::ok(format!(
        "Started gauntlet {} (round 1 of {}). The builder is running; end this turn and wait for its completion notice, then call advance. Do not poll or use sleep.",
        gauntlet.id.0, gauntlet.max_rounds
    )))
}
