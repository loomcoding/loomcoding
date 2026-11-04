//! Herramienta `gauntlet`: un constructor y un crítico ciego e independiente, en secuencia y con
//! rondas acotadas. Imita a `development_workflow`: la gobierna el agente padre, los hijos son
//! agentes locales registrados y la decisión final es de la persona.
//! Spec: `docs/specs/2026-11-04-loomcoding-gauntlet-minimo.md`.

use super::{ExecutionMode, Tool, ToolCtx, ToolOutput};
use crate::error::{CoreError, Result};
use crate::gauntlet::{Gauntlet, GauntletPhase};
use crate::question::{Answer, Question, QuestionOption};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

mod advance;
mod decide;
mod helpers;
mod start;
mod status;

use advance::*;
use decide::*;
use helpers::*;
use start::*;
use status::*;

#[cfg(test)]
mod tests;

type Store = std::sync::Arc<dyn crate::session::SessionStore>;
type Launcher = std::sync::Arc<dyn crate::subagent::SubagentLauncher>;

pub struct GauntletTool;

#[async_trait]
impl Tool for GauntletTool {
    fn name(&self) -> &str {
        "gauntlet"
    }
    fn description(&self) -> &str {
        "Polish a piece of work with a builder and an independent blind critic. Builder and critic must be two distinct local agent names. Use it only when the user asks for this method. After start, end the turn and wait for the completion notice; do not poll or sleep. Call advance each time a child finishes. Never edit the artifact as the parent. Actions: start, advance, accept (final user decision), cancel, status."
    }
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{
            "action":{"type":"string","enum":["start","advance","accept","cancel","status"]},
            "goal":{"type":"string","maxLength":8000,"description":"What the piece must achieve."},
            "artifact":{"type":"string","maxLength":512,"description":"Project path of the text file to improve."},
            "bar":{"type":"array","items":{"type":"string","maxLength":512},"minItems":1,"maxItems":8,"description":"Project paths of the text files that set the quality bar."},
            "builder":{"type":"string","maxLength":64,"description":"Local agent name; must differ from critic."},
            "critic":{"type":"string","maxLength":64,"description":"Local agent name; must differ from builder."},
            "max_rounds":{"type":"integer","minimum":1,"maximum":5,"description":"Build rounds before stopping (default 2)."},
            "check":{"type":"string","maxLength":1000,"description":"Optional objective check (a shell command, for example `cargo check`). Run after each builder round; if it fails its output goes back to the builder and no critic is spent. It needs the same permission as bash."},
            "hide_bar":{"type":"boolean","description":"Do not tell the builder where the bar is (use it for code, where a readable reference gets copied). Default false."}
        },"required":["action"]})
    }
    fn execution_mode(&self, _: &serde_json::Value) -> ExecutionMode {
        ExecutionMode::Exclusive
    }
    async fn run(
        &self,
        ctx: ToolCtx,
        input: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolOutput> {
        let store = ctx
            .store
            .as_ref()
            .ok_or_else(|| CoreError::Tool("gauntlet storage unavailable".into()))?;
        let launcher = ctx
            .subagents
            .as_ref()
            .ok_or_else(|| CoreError::Tool("subagents unavailable".into()))?;
        match input["action"].as_str().unwrap_or_default() {
            "start" => action_start(&ctx, &input, &cancel, store, launcher).await,
            "status" => action_status(&ctx, store, launcher).await,
            action @ ("cancel" | "accept" | "advance") => {
                let gauntlet = store
                    .active_gauntlet(ctx.session)
                    .await?
                    .ok_or_else(|| CoreError::Tool("no active gauntlet".into()))?;
                match action {
                    "cancel" => action_cancel(&ctx, store, launcher, gauntlet).await,
                    "accept" => action_accept(&ctx, &cancel, store, gauntlet).await,
                    _ => action_advance(&ctx, &cancel, store, launcher, gauntlet).await,
                }
            }
            _ => Err(CoreError::Tool("unknown gauntlet action".into())),
        }
    }
}
