//! Acciones de la persona: `cancel` y `accept`.

use super::*;

/// Cierra el gauntlet como cancelado y pide al hijo en marcha que pare.
pub(super) async fn action_cancel(
    ctx: &ToolCtx,
    store: &Store,
    launcher: &Launcher,
    mut gauntlet: Gauntlet,
) -> Result<ToolOutput> {
    let running = gauntlet.active_child.take();
    gauntlet.cancel().map_err(problem)?;
    save(&ctx.bus, store.as_ref(), gauntlet, "user", "cancelled").await?;
    if let Some(child) = running {
        // Parar el hijo no está garantizado ni es condición de la cancelación: se pide y se sigue.
        let _ = launcher.cancel(ctx.session, child).await;
    }
    Ok(ToolOutput::ok("Gauntlet cancelled."))
}

/// Pide a la persona la aceptación final. Solo un «sí» explícito completa; todo lo demás cancela.
pub(super) async fn action_accept(
    ctx: &ToolCtx,
    cancel: &CancellationToken,
    store: &Store,
    mut gauntlet: Gauntlet,
) -> Result<ToolOutput> {
    if gauntlet.phase != GauntletPhase::AwaitingFinalAcceptance {
        return Err(CoreError::Tool(
            "gauntlet is not awaiting final acceptance".into(),
        ));
    }
    let current = artifact_hash(&ctx.cwd, &gauntlet.artifact)?;
    if gauntlet.presented_sha256.as_deref() != Some(current.as_str()) {
        return Ok(ToolOutput::err(
            "The artifact changed after it was presented to the user; cancel this gauntlet and start a new one.",
        ));
    }
    let question = [Question {
        header: "Final acceptance".into(),
        question: format!(
            "Accept `{}` as finished? The independent critic found no gap against the quality bar, confirmed with the order swapped.",
            gauntlet.artifact
        ),
        options: vec![
            QuestionOption {
                label: "Accept and complete".into(),
                description: String::new(),
            },
            QuestionOption {
                label: "Cancel".into(),
                description: String::new(),
            },
        ],
        multi_select: false,
    }];
    let answers = tokio::select! { _ = cancel.cancelled() => None, a = ctx.permission.ask_user(&question) => a };
    let accepted = answers
        .as_ref()
        .and_then(|a| a.first())
        .is_some_and(|a: &Answer| a.selected == ["Accept and complete"]);
    if accepted {
        gauntlet.accept().map_err(problem)?;
    } else {
        gauntlet.cancel().map_err(problem)?;
    }
    save(
        &ctx.bus,
        store.as_ref(),
        gauntlet,
        "user",
        if accepted {
            "final acceptance"
        } else {
            "final acceptance declined"
        },
    )
    .await?;
    Ok(ToolOutput::ok(if accepted {
        "Gauntlet completed."
    } else {
        "Gauntlet cancelled; the result was not accepted."
    }))
}
