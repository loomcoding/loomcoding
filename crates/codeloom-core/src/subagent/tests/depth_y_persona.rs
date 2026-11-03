use super::*;

#[test]
fn a_full_persona_and_a_full_skills_prompt_both_reach_the_child_untouched() {
    let persona = "p".repeat(MAX_PROFILE_PROMPT_BYTES);
    let skills = "s".repeat(crate::skills::MAX_SKILL_PROMPT_TOKENS * 4);
    let sistema = compose_child_system(Some(persona.clone()), &skills).unwrap();
    assert!(sistema.starts_with(&persona));
    assert!(sistema.ends_with(&skills));
    assert_eq!(sistema.len(), persona.len() + 2 + skills.len());
}

#[test]
fn the_persona_is_capped_on_its_own_and_without_skills_it_is_returned_as_is() {
    let larga = "p".repeat(MAX_PROFILE_PROMPT_BYTES * 2);
    let sistema = compose_child_system(Some(larga.clone()), "").unwrap();
    // El recorte añade su marcador, que cabe en la holgura.
    assert!(sistema.len() <= MAX_PROFILE_PROMPT_BYTES + super::TRUNCATION_MARKER_SLACK);
    assert!(sistema.contains("bytes omitted"));
    assert_eq!(compose_child_system(None, ""), None);
    assert_eq!(
        compose_child_system(None, "skills").as_deref(),
        Some("skills")
    );
}

#[test]
fn with_the_default_ceiling_the_child_never_delegates_even_if_its_profile_asks() {
    let raiz = root_levels(1);
    assert!(can_delegate(raiz));
    let hijo = child_levels(raiz, Some(3));
    assert_eq!(hijo, 1);
    assert!(!can_delegate(hijo));
    assert!(depth_clipped(raiz, Some(3)));
    assert!(!depth_clipped(raiz, Some(1)));
    assert!(!depth_clipped(raiz, None));
}

#[test]
fn ceiling_two_and_a_profile_with_two_levels_give_a_grandchild_that_is_a_leaf() {
    let raiz = root_levels(2);
    let hijo = child_levels(raiz, Some(2));
    assert!(can_delegate(hijo));
    let nieto = child_levels(hijo, None);
    assert_eq!(nieto, 1);
    assert!(!can_delegate(nieto));
}

#[test]
fn ceiling_three_gives_a_chain_whose_last_member_is_a_leaf() {
    let hijo = child_levels(root_levels(3), Some(3));
    let nieto = child_levels(hijo, Some(2));
    assert!(can_delegate(hijo) && can_delegate(nieto));
    let bisnieto = child_levels(nieto, Some(3));
    assert_eq!(bisnieto, 1);
    assert!(!can_delegate(bisnieto));
}

#[test]
fn a_profile_without_max_depth_is_a_leaf_even_with_a_high_ceiling() {
    let hijo = child_levels(root_levels(2), None);
    assert_eq!(hijo, 1);
    assert!(!can_delegate(hijo));
    assert!(!depth_clipped(root_levels(2), None));
}

#[test]
fn a_zero_in_the_profile_still_counts_as_a_leaf() {
    assert_eq!(child_levels(root_levels(3), Some(0)), 1);
}

#[tokio::test]
async fn runs_a_child_and_queues_its_parent_notice_without_recursive_tool() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        SqliteStore::open(dir.path().join("subagents.db").to_str().unwrap())
            .await
            .unwrap(),
    );
    let parent = store.create_session("/project").await.unwrap();
    let provider: Arc<dyn Provider> = Arc::new(ScriptedProvider {
        script: vec![
            StreamChunk::TextDelta("child result".into()),
            StreamChunk::Done(FinishReason::Stop),
        ],
    });
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(SubagentTool));
    let jobs = Arc::new(LocalJobRegistry::default());
    let runtime = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store.clone(),
        Arc::new(tools),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(CodeLoomConfig {
            provider: ProviderConfig {
                kind: ProviderKind::OpenAiCompat,
                base_url: "http://localhost".into(),
                api_key_env: "KEY".into(),
            },
            model: "test".into(),
            max_steps: 2,
            effort: None,
        })),
        dir.path().to_path_buf(),
        jobs.clone(),
        EventBus::new(64),
    );

    assert!(runtime.tools.get("subagent").is_none());
    assert!(runtime.tools.get("subagent_control").is_none());
    assert!(runtime.tools.get("subagent_list").is_none());
    let (child, job) = runtime
        .start(
            parent,
            "research".into(),
            "do work".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(runtime
        .control(SessionId(999), child, "interrupt", None)
        .await
        .is_err());
    let outcome = jobs.wait(parent, &job).await.unwrap();

    assert_eq!(outcome.status, JobStatus::Completed);
    assert!(store.messages(child).await.unwrap().iter().any(|message| {
        message.role == crate::message::Role::Assistant && message.text().contains("child result")
    }));
    assert!(runtime
        .deliver_notices(parent)
        .await
        .unwrap()
        .iter()
        .any(|message| message.text().contains("child result")));
    assert!(runtime.deliver_notices(parent).await.unwrap().is_empty());
    assert!(runtime
        .control(parent, child, "interrupt", None)
        .await
        .is_err());
}
