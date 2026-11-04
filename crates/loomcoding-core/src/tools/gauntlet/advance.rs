//! Acción `advance`: localiza el hijo terminado y reparte según la fase.

use super::*;
use crate::gauntlet::{parse_verdict, Next, Verdict};
use crate::task::TaskState;

pub(super) async fn action_advance(
    ctx: &ToolCtx,
    cancel: &CancellationToken,
    store: &Store,
    launcher: &Launcher,
    mut gauntlet: Gauntlet,
) -> Result<ToolOutput> {
    let child = gauntlet
        .active_child
        .ok_or_else(|| CoreError::Tool("gauntlet has no active child".into()))?;
    let descriptor = launcher
        .list(ctx.session)
        .await?
        .into_iter()
        .find(|item| item.child == child)
        .ok_or_else(|| CoreError::Tool("gauntlet child is missing".into()))?;
    let state = descriptor.status.state;
    if !state.is_terminal() {
        return Ok(ToolOutput::err("gauntlet child is still running"));
    }
    if state != TaskState::Completed {
        let reason = format!("gauntlet child ended in {state:?}");
        if matches!(state, TaskState::Canceled | TaskState::Rejected) {
            gauntlet.cancel().map_err(problem)?;
            gauntlet.active_child = None;
            save(&ctx.bus, store.as_ref(), gauntlet, "coordinator", &reason).await?;
        } else {
            return fail_gauntlet(&ctx.bus, store.as_ref(), gauntlet, &reason).await;
        }
        return Ok(ToolOutput::err(
            "gauntlet child did not complete successfully; nothing was advanced",
        ));
    }
    let Some(report) = descriptor
        .result_text()
        .filter(|report| !report.trim().is_empty())
    else {
        return fail_gauntlet(
            &ctx.bus,
            store.as_ref(),
            gauntlet,
            "child completed without a result; no further round was started",
        )
        .await;
    };
    gauntlet.active_child = None;
    match gauntlet.phase {
        GauntletPhase::Building | GauntletPhase::Rebuilding => {
            let order = gauntlet.begin_judging(draw_seed()).map_err(problem)?;
            let round = gauntlet.round;
            let gauntlet = save(
                &ctx.bus,
                store.as_ref(),
                gauntlet,
                "builder",
                &format!("builder finished round {round}"),
            )
            .await?;
            launch_critic(
                &ctx.bus,
                store.as_ref(),
                launcher.as_ref(),
                gauntlet,
                order,
                cancel,
            )
            .await?;
            Ok(ToolOutput::ok(format!(
                "Builder finished round {round}; the critic is judging. End this turn and wait for its completion notice, then call advance."
            )))
        }
        GauntletPhase::Judging => {
            advance_judging(ctx, cancel, store, launcher, gauntlet, &report).await
        }
        _ => Err(CoreError::Tool(
            "gauntlet cannot advance from this phase".into(),
        )),
    }
}

/// El crítico terminó: se lee su veredicto con el orden que se le mostró y se decide el paso.
async fn advance_judging(
    ctx: &ToolCtx,
    cancel: &CancellationToken,
    store: &Store,
    launcher: &Launcher,
    mut gauntlet: Gauntlet,
    report: &str,
) -> Result<ToolOutput> {
    let order = gauntlet
        .rounds
        .last()
        .map(|record| record.order)
        .ok_or_else(|| CoreError::Tool("gauntlet has no judging in progress".into()))?;
    let verdict = parse_verdict(report, order);
    let detail = match &verdict {
        Verdict::Gap { biggest_gap } => format!("gap: {biggest_gap}"),
        Verdict::NoGap => "no gap".to_owned(),
        Verdict::Malformed => "unreadable verdict".to_owned(),
    };
    let next = gauntlet.apply_verdict(verdict).map_err(problem)?;
    match next {
        Next::Rebuild => {
            let gap = gauntlet.last_gap.clone().unwrap_or_default();
            let gauntlet = save(&ctx.bus, store.as_ref(), gauntlet, "critic", &detail).await?;
            let prompt = rebuild_prompt(&gauntlet, &gap);
            let gauntlet = launch_builder(
                &ctx.bus,
                store.as_ref(),
                launcher.as_ref(),
                gauntlet,
                prompt,
                cancel,
            )
            .await?;
            Ok(ToolOutput::ok(format!(
                "The critic found a gap; round {} of {} started with the builder. End this turn and wait for its completion notice, then call advance.",
                gauntlet.round, gauntlet.max_rounds
            )))
        }
        Next::Recheck(swapped) => {
            let gauntlet = save(&ctx.bus, store.as_ref(), gauntlet, "critic", &detail).await?;
            launch_critic(
                &ctx.bus,
                store.as_ref(),
                launcher.as_ref(),
                gauntlet,
                swapped,
                cancel,
            )
            .await?;
            Ok(ToolOutput::ok(
                "The critic saw no gap; a fresh critic is re-judging with the order swapped to rule out position bias. End this turn and wait for its completion notice, then call advance.",
            ))
        }
        Next::Accept => {
            let hash = artifact_hash(&ctx.cwd, &gauntlet.artifact)?;
            gauntlet.presented_sha256 = Some(hash);
            save(
                &ctx.bus,
                store.as_ref(),
                gauntlet,
                "critic",
                "no gap, confirmed with the order swapped",
            )
            .await?;
            Ok(ToolOutput::ok(
                "Both judgments found no gap. Present the result to the user and call accept to ask for the final decision.",
            ))
        }
        Next::Stop => {
            save(&ctx.bus, store.as_ref(), gauntlet, "critic", &detail).await?;
            Ok(ToolOutput::ok(
                "The round limit was reached with a gap remaining. Report the remaining gap to the user and let them decide; do not start another round.",
            ))
        }
        Next::Inconclusive => {
            save(&ctx.bus, store.as_ref(), gauntlet, "critic", &detail).await?;
            Ok(ToolOutput::ok(
                "The critic's verdict was unreadable or contradicted itself when the order was swapped. Report this to the user and let them decide.",
            ))
        }
    }
}
