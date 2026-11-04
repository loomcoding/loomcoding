use super::steps::{final_round, round, run_configured, run_with_limits};
use super::*;
use crate::config::LimitsConfig;
use crate::permission::{Decision, PermissionRequest, PermissionResponder, PermissionService};
use crate::question::{Answer, Question};

/// Responder de prueba que contesta a la pregunta del tope con respuestas fijas y cuenta cuántas veces se le preguntó.
struct Contesta {
    respuestas: Mutex<VecDeque<Option<&'static str>>>,
    por_defecto: Option<&'static str>,
    preguntas: Arc<AtomicUsize>,
}

#[async_trait]
impl PermissionResponder for Contesta {
    async fn decide(&self, _req: &PermissionRequest) -> Decision {
        Decision::Allow
    }

    async fn ask(&self, questions: &[Question]) -> Option<Vec<Answer>> {
        self.preguntas.fetch_add(1, Ordering::SeqCst);
        assert_eq!(questions[0].header, "Step limit");
        let labels: Vec<_> = questions[0]
            .options
            .iter()
            .map(|o| o.label.as_str())
            .collect();
        assert_eq!(labels, ["Continue", "Stop"]);
        let respuesta = self
            .respuestas
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(self.por_defecto)?;
        Some(vec![Answer {
            question: questions[0].question.clone(),
            selected: vec![respuesta.into()],
        }])
    }
}

fn servicio(
    respuestas: Vec<Option<&'static str>>,
    por_defecto: Option<&'static str>,
) -> (Arc<PermissionService>, Arc<AtomicUsize>) {
    let preguntas = Arc::new(AtomicUsize::new(0));
    let responder = Contesta {
        respuestas: Mutex::new(respuestas.into()),
        por_defecto,
        preguntas: preguntas.clone(),
    };
    (
        Arc::new(PermissionService::with_responder(Box::new(responder))),
        preguntas,
    )
}

#[tokio::test]
async fn step_limit_asks_and_continues_when_the_person_agrees() {
    let (permiso, preguntas) = servicio(vec![Some("Continue")], None);
    let scripts = vec![round("edit", 0), final_round()];
    let (outcome, _) = run_with_limits(scripts, 1, LimitsConfig::default(), permiso).await;
    assert_eq!(outcome.final_text, "done");
    assert_eq!(preguntas.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn step_limit_stops_when_the_person_declines() {
    let (permiso, preguntas) = servicio(vec![Some("Stop")], None);
    let scripts = vec![round("edit", 0), final_round()];
    let (outcome, left) = run_with_limits(scripts, 1, LimitsConfig::default(), permiso).await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(left, 1);
    assert_eq!(preguntas.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn step_limit_stops_without_anyone_to_ask() {
    let scripts = vec![round("edit", 0), final_round()];
    let permiso = Arc::new(PermissionService::allow_all());
    let (outcome, left) = run_with_limits(scripts, 1, LimitsConfig::default(), permiso).await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(left, 1);
}

#[tokio::test]
async fn a_subagent_never_asks_at_the_step_limit() {
    let (permiso, preguntas) = servicio(vec![], Some("Continue"));
    let scripts = vec![round("edit", 0), final_round()];
    let (outcome, left) = run_configured(scripts, 1, LimitsConfig::default(), permiso, true).await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(left, 1);
    assert_eq!(preguntas.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn an_unrecognised_answer_counts_as_stop() {
    let (permiso, _) = servicio(vec![Some("Quizá")], None);
    let scripts = vec![round("edit", 0), final_round()];
    let (outcome, _) = run_with_limits(scripts, 1, LimitsConfig::default(), permiso).await;
    assert_eq!(outcome.final_text, "");
}

#[tokio::test]
async fn step_limit_extensions_are_capped() {
    let (permiso, preguntas) = servicio(vec![], Some("Continue"));
    let scripts: Vec<_> = (0..12).map(|n| round("edit", n)).collect();
    let (outcome, left) = run_with_limits(scripts, 1, LimitsConfig::default(), permiso).await;
    assert_eq!(outcome.final_text, "");
    assert_eq!(
        preguntas.load(Ordering::SeqCst),
        stop::MAX_STEP_EXTENSIONS,
        "se pregunta solo mientras queden prórrogas"
    );
    // Un paso inicial más una prórroga de un paso cada vez.
    assert_eq!(left, 12 - (1 + stop::MAX_STEP_EXTENSIONS));
}

#[tokio::test]
async fn the_read_budget_asks_too_and_a_yes_raises_only_that_cap() {
    let (permiso, preguntas) = servicio(vec![Some("Continue")], None);
    let mut scripts: Vec<_> = (0..4).map(|n| round("view", n)).collect();
    scripts.push(final_round());
    let limits = LimitsConfig {
        max_read_steps: Some(2),
        ..LimitsConfig::default()
    };
    let (outcome, _) = run_with_limits(scripts, 30, limits, permiso).await;
    assert_eq!(preguntas.load(Ordering::SeqCst), 1);
    // Tope de lectura 2 + prórroga de `max_steps` (30) rondas: sobra para las cuatro lecturas y el texto.
    assert_eq!(outcome.final_text, "done");
}
