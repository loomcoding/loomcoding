//! Acción `status`: consulta sin avanzar nada.

use super::*;

pub(super) async fn action_status(
    ctx: &ToolCtx,
    store: &Store,
    launcher: &Launcher,
) -> Result<ToolOutput> {
    let gauntlet = match store.active_gauntlet(ctx.session).await? {
        Some(gauntlet) => gauntlet,
        None => store
            .latest_gauntlet(ctx.session)
            .await?
            .ok_or_else(|| CoreError::Tool("no gauntlet exists".into()))?,
    };
    let mut child_state = None;
    let mut next_action = if gauntlet.phase.is_terminal() {
        "finished; report the outcome to the user"
    } else if gauntlet.phase == GauntletPhase::AwaitingFinalAcceptance {
        "call accept to ask the user for the final decision"
    } else {
        "call advance"
    };
    if let Some(child) = gauntlet.active_child {
        if let Some(descriptor) = launcher
            .list(ctx.session)
            .await?
            .into_iter()
            .find(|item| item.child == child)
        {
            child_state = Some(descriptor.status.state.proto_name());
            next_action = if descriptor.status.state.is_terminal() {
                "the child finished; call advance"
            } else {
                "wait for this subagent; it is still running"
            };
        }
    }
    Ok(ToolOutput::ok(
        serde_json::to_string(&serde_json::json!({
            "gauntlet": gauntlet.summary(),
            "child_state": child_state,
            "next_action": next_action
        }))
        .unwrap_or_default(),
    ))
}
