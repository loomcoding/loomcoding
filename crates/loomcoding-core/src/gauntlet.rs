//! Estado durable del Gauntlet mínimo: un constructor, un crítico ciego y un bucle acotado.
//!
//! Cabe señalar que este módulo es puro: sin E/S, sin red y sin reloj. El sorteo del orden A/B
//! entra por semilla inyectada y la hora, por parámetro, de modo que todo es determinista en
//! pruebas. Las fases son propias de Loomcoding; cada hijo sigue siendo una tarea A2A normal.
//! Spec: `docs/specs/2026-11-04-loomcoding-gauntlet-minimo.md`.

use crate::session::SessionId;
use serde::{Deserialize, Serialize};

/// Rondas por defecto cuando la llamada no indica ninguna.
pub const DEFAULT_MAX_ROUNDS: u8 = 2;
/// Tope duro de rondas: sin él el flujo sería un bucle caro sin cota.
pub const MAX_ROUNDS_LIMIT: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GauntletId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GauntletPhase {
    /// El constructor trabaja la pieza (primera ronda).
    Building,
    /// El crítico juzga; también durante la comprobación de sesgo.
    Judging,
    /// El constructor corrige a partir de la brecha del crítico.
    Rebuilding,
    AwaitingFinalAcceptance,
    Completed,
    /// Se agotaron las rondas con brecha: decide la persona.
    Stopped,
    /// Veredicto malformado o contradictorio: decide la persona.
    Inconclusive,
    Cancelled,
    Failed,
}

impl GauntletPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Building => "building",
            Self::Judging => "judging",
            Self::Rebuilding => "rebuilding",
            Self::AwaitingFinalAcceptance => "awaiting_final_acceptance",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::Inconclusive => "inconclusive",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Stopped | Self::Inconclusive | Self::Cancelled | Self::Failed
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use GauntletPhase::*;
        if self.is_terminal() {
            return false;
        }
        self == next
            || matches!(
                (self, next),
                (Building, Judging | Cancelled | Failed)
                    | (
                        Judging,
                        Rebuilding
                            | AwaitingFinalAcceptance
                            | Stopped
                            | Inconclusive
                            | Cancelled
                            | Failed
                    )
                    | (Rebuilding, Judging | Cancelled | Failed)
                    | (AwaitingFinalAcceptance, Completed | Cancelled | Failed)
            )
    }
}

impl std::str::FromStr for GauntletPhase {
    type Err = serde_json::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        serde_json::from_value(serde_json::Value::String(value.to_owned()))
    }
}

/// Qué va en la posición «A» que ve el crítico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbOrder {
    ArtifactFirst,
    BarFirst,
}

impl AbOrder {
    pub fn from_seed(seed: u64) -> Self {
        if seed.is_multiple_of(2) {
            Self::ArtifactFirst
        } else {
            Self::BarFirst
        }
    }

    pub fn swapped(self) -> Self {
        match self {
            Self::ArtifactFirst => Self::BarFirst,
            Self::BarFirst => Self::ArtifactFirst,
        }
    }
}

/// Veredicto ya interpretado del crítico. Lo malformado nunca equivale a aprobación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Gap { biggest_gap: String },
    NoGap,
    Malformed,
}

/// Cota de la brecha que se conserva y se pasa al constructor (decálogo de contexto y coste).
pub const MAX_GAP_BYTES: usize = 1_000;
/// Del informe del crítico solo se escanea la cola: el veredicto va al final.
pub const MAX_REPORT_BYTES: usize = 20_000;

#[derive(Deserialize)]
struct RawVerdict {
    preferred: Option<String>,
    biggest_gap: Option<String>,
}

fn cut_on_boundary(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn tail_on_boundary(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// Interpreta el informe del crítico ciego: el último objeto JSON con campo `preferred`.
///
/// El crítico no sabe cuál de los dos candidatos es la barra: dice cuál prefiere (`A`, `B` o
/// `tie`). Quien conoce el `order` traduce: si prefiere la barra, hay brecha y debe describirla en
/// `biggest_gap`; si prefiere el artefacto o no distingue, no hay brecha.
///
/// Todo lo que no sea inequívoco es [`Verdict::Malformed`], nunca aprobación: preferencia
/// desconocida, brecha sin texto cuando prefiere la barra o JSON truncado.
pub fn parse_verdict(report: &str, order: AbOrder) -> Verdict {
    let window = tail_on_boundary(report, MAX_REPORT_BYTES);
    let (artifact_label, bar_label) = match order {
        AbOrder::ArtifactFirst => ("A", "B"),
        AbOrder::BarFirst => ("B", "A"),
    };
    for (start, _) in window.match_indices('{').rev() {
        let mut stream =
            serde_json::Deserializer::from_str(&window[start..]).into_iter::<RawVerdict>();
        let Some(Ok(raw)) = stream.next() else {
            continue;
        };
        let Some(preferred) = raw.preferred else {
            continue;
        };
        let gap = raw.biggest_gap.filter(|text| !text.trim().is_empty());
        let preferred = preferred.trim();
        return if preferred == bar_label {
            match gap {
                Some(text) => Verdict::Gap {
                    biggest_gap: cut_on_boundary(text.trim(), MAX_GAP_BYTES).to_owned(),
                },
                None => Verdict::Malformed,
            }
        } else if preferred == artifact_label || preferred == "tie" {
            Verdict::NoGap
        } else {
            Verdict::Malformed
        };
    }
    Verdict::Malformed
}

/// Lo que el coordinador debe hacer a continuación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Volver al constructor con la brecha.
    Rebuild,
    /// Repetir el juicio con este orden (comprobación de sesgo de posición).
    Recheck(AbOrder),
    /// Presentar el resultado a la persona.
    Accept,
    /// Rondas agotadas con brecha.
    Stop,
    /// No hay veredicto fiable.
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundRecord {
    pub round: u8,
    pub order: AbOrder,
    /// `true` si es la repetición con el orden invertido tras un primer `no_gap`.
    pub bias_check: bool,
    pub verdict: Option<Verdict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GauntletSummary {
    pub id: GauntletId,
    pub parent: SessionId,
    pub phase: GauntletPhase,
    pub round: u8,
    pub max_rounds: u8,
    pub builder: String,
    pub critic: String,
    pub active_child: Option<SessionId>,
    pub updated_at_ms: i64,
}

/// Entrada de la bitácora, solo de añadir: una por cada cambio de fase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GauntletEvent {
    pub sequence: i64,
    pub gauntlet: GauntletId,
    pub from: Option<GauntletPhase>,
    pub to: GauntletPhase,
    pub actor: String,
    pub detail: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GauntletProblem {
    #[error("el constructor y el crítico deben ser agentes distintos (`{0}`)")]
    SameAgent(String),
    #[error("la barra necesita al menos una ruta")]
    EmptyBar,
    #[error("el campo `{0}` no puede estar vacío")]
    Blank(&'static str),
    #[error("`max_rounds` debe estar entre 1 y {MAX_ROUNDS_LIMIT}, no {0}")]
    RoundsOutOfRange(u8),
    #[error("transición no válida: {from} -> {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },
}

/// Datos de entrada de un gauntlet nuevo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGauntlet {
    pub id: GauntletId,
    pub parent: SessionId,
    pub goal: String,
    pub artifact: String,
    pub bar: Vec<String>,
    pub builder: String,
    pub critic: String,
    pub max_rounds: Option<u8>,
    pub now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gauntlet {
    pub id: GauntletId,
    pub parent: SessionId,
    pub goal: String,
    pub artifact: String,
    pub bar: Vec<String>,
    pub builder: String,
    pub critic: String,
    pub max_rounds: u8,
    /// Ronda de construcción en curso, desde 1.
    pub round: u8,
    pub phase: GauntletPhase,
    pub version: i64,
    pub active_child: Option<SessionId>,
    pub builder_children: Vec<SessionId>,
    pub critic_children: Vec<SessionId>,
    pub rounds: Vec<RoundRecord>,
    /// Última brecha, la única información que recibe el constructor al rehacer.
    pub last_gap: Option<String>,
    /// Huella del artefacto cuando se presentó a la persona; la aceptación se liga a ella.
    #[serde(default)]
    pub presented_sha256: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

fn not_blank(value: &str, field: &'static str) -> Result<(), GauntletProblem> {
    if value.trim().is_empty() {
        Err(GauntletProblem::Blank(field))
    } else {
        Ok(())
    }
}

impl Gauntlet {
    pub fn new(data: NewGauntlet) -> Result<Self, GauntletProblem> {
        not_blank(&data.goal, "goal")?;
        not_blank(&data.artifact, "artifact")?;
        not_blank(&data.builder, "builder")?;
        not_blank(&data.critic, "critic")?;
        if data.builder == data.critic {
            return Err(GauntletProblem::SameAgent(data.builder));
        }
        if data.bar.is_empty() || data.bar.iter().any(|path| path.trim().is_empty()) {
            return Err(GauntletProblem::EmptyBar);
        }
        let max_rounds = data.max_rounds.unwrap_or(DEFAULT_MAX_ROUNDS);
        if !(1..=MAX_ROUNDS_LIMIT).contains(&max_rounds) {
            return Err(GauntletProblem::RoundsOutOfRange(max_rounds));
        }
        Ok(Self {
            id: data.id,
            parent: data.parent,
            goal: data.goal,
            artifact: data.artifact,
            bar: data.bar,
            builder: data.builder,
            critic: data.critic,
            max_rounds,
            round: 1,
            phase: GauntletPhase::Building,
            version: 0,
            active_child: None,
            builder_children: Vec::new(),
            critic_children: Vec::new(),
            rounds: Vec::new(),
            last_gap: None,
            presented_sha256: None,
            created_at_ms: data.now_ms,
            updated_at_ms: data.now_ms,
        })
    }

    fn go(&mut self, next: GauntletPhase) -> Result<(), GauntletProblem> {
        if !self.phase.can_transition_to(next) {
            return Err(GauntletProblem::InvalidTransition {
                from: self.phase.as_str(),
                to: next.as_str(),
            });
        }
        self.phase = next;
        Ok(())
    }

    /// Pasa a juzgar la ronda en curso y sortea el orden A/B con la semilla dada.
    pub fn begin_judging(&mut self, seed: u64) -> Result<AbOrder, GauntletProblem> {
        if !matches!(
            self.phase,
            GauntletPhase::Building | GauntletPhase::Rebuilding
        ) {
            return Err(GauntletProblem::InvalidTransition {
                from: self.phase.as_str(),
                to: GauntletPhase::Judging.as_str(),
            });
        }
        self.go(GauntletPhase::Judging)?;
        let order = AbOrder::from_seed(seed);
        self.rounds.push(RoundRecord {
            round: self.round,
            order,
            bias_check: false,
            verdict: None,
        });
        Ok(order)
    }

    /// Aplica el veredicto del juicio en curso y dice qué toca hacer.
    pub fn apply_verdict(&mut self, verdict: Verdict) -> Result<Next, GauntletProblem> {
        if self.phase != GauntletPhase::Judging {
            return Err(GauntletProblem::InvalidTransition {
                from: self.phase.as_str(),
                to: "verdict",
            });
        }
        let in_bias_check = self.rounds.last().is_some_and(|record| record.bias_check);
        let order = self.rounds.last().map(|record| record.order);
        if let Some(record) = self.rounds.last_mut() {
            record.verdict = Some(verdict.clone());
        }
        match verdict {
            Verdict::Malformed => {
                self.go(GauntletPhase::Inconclusive)?;
                Ok(Next::Inconclusive)
            }
            Verdict::Gap { .. } if in_bias_check => {
                self.go(GauntletPhase::Inconclusive)?;
                Ok(Next::Inconclusive)
            }
            Verdict::Gap { biggest_gap } => {
                if self.round >= self.max_rounds {
                    self.go(GauntletPhase::Stopped)?;
                    return Ok(Next::Stop);
                }
                self.go(GauntletPhase::Rebuilding)?;
                self.round += 1;
                self.last_gap = Some(biggest_gap);
                Ok(Next::Rebuild)
            }
            Verdict::NoGap if in_bias_check => {
                self.go(GauntletPhase::AwaitingFinalAcceptance)?;
                Ok(Next::Accept)
            }
            Verdict::NoGap => {
                let swapped = order.unwrap_or(AbOrder::ArtifactFirst).swapped();
                self.rounds.push(RoundRecord {
                    round: self.round,
                    order: swapped,
                    bias_check: true,
                    verdict: None,
                });
                Ok(Next::Recheck(swapped))
            }
        }
    }

    /// Decisión humana afirmativa: única vía hacia `Completed`.
    pub fn accept(&mut self) -> Result<(), GauntletProblem> {
        self.go(GauntletPhase::Completed)
    }

    pub fn cancel(&mut self) -> Result<(), GauntletProblem> {
        self.go(GauntletPhase::Cancelled)
    }

    pub fn fail(&mut self) -> Result<(), GauntletProblem> {
        self.go(GauntletPhase::Failed)
    }

    /// Vista corta para la TUI y para `status`: sin objetivo ni rutas.
    pub fn summary(&self) -> GauntletSummary {
        GauntletSummary {
            id: self.id,
            parent: self.parent,
            phase: self.phase,
            round: self.round,
            max_rounds: self.max_rounds,
            builder: self.builder.clone(),
            critic: self.critic.clone(),
            active_child: self.active_child,
            updated_at_ms: self.updated_at_ms,
        }
    }

    /// Registra el hijo recién lanzado según la fase que lo ha motivado.
    pub fn attach_child(&mut self, child: SessionId) {
        self.active_child = Some(child);
        match self.phase {
            GauntletPhase::Building | GauntletPhase::Rebuilding => {
                self.builder_children.push(child)
            }
            GauntletPhase::Judging => self.critic_children.push(child),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nuevo(max_rounds: Option<u8>) -> Gauntlet {
        Gauntlet::new(NewGauntlet {
            id: GauntletId(1),
            parent: SessionId(7),
            goal: "pulir la portada".into(),
            artifact: "out/portada.md".into(),
            bar: vec!["ref/portada.md".into()],
            builder: "constructor".into(),
            critic: "critico".into(),
            max_rounds,
            now_ms: 10,
        })
        .unwrap()
    }

    fn en_juicio(max_rounds: Option<u8>) -> Gauntlet {
        let mut g = nuevo(max_rounds);
        g.begin_judging(0).unwrap();
        g
    }

    fn brecha() -> Verdict {
        Verdict::Gap {
            biggest_gap: "los márgenes".into(),
        }
    }

    #[test]
    fn rounds_default_to_two_and_are_bounded_to_five() {
        assert_eq!(nuevo(None).max_rounds, 2);
        assert_eq!(nuevo(Some(5)).max_rounds, 5);
        for fuera in [0u8, 6] {
            let mut datos = NewGauntlet {
                id: GauntletId(1),
                parent: SessionId(7),
                goal: "g".into(),
                artifact: "a".into(),
                bar: vec!["b".into()],
                builder: "x".into(),
                critic: "y".into(),
                max_rounds: Some(fuera),
                now_ms: 0,
            };
            assert_eq!(
                Gauntlet::new(datos.clone()).unwrap_err(),
                GauntletProblem::RoundsOutOfRange(fuera)
            );
            datos.max_rounds = None;
            assert!(Gauntlet::new(datos).is_ok());
        }
    }

    #[test]
    fn builder_and_critic_must_be_different_agents() {
        let datos = NewGauntlet {
            id: GauntletId(1),
            parent: SessionId(7),
            goal: "g".into(),
            artifact: "a".into(),
            bar: vec!["b".into()],
            builder: "mismo".into(),
            critic: "mismo".into(),
            max_rounds: None,
            now_ms: 0,
        };
        assert_eq!(
            Gauntlet::new(datos).unwrap_err(),
            GauntletProblem::SameAgent("mismo".into())
        );
    }

    #[test]
    fn an_empty_bar_or_blank_fields_are_rejected() {
        let base = NewGauntlet {
            id: GauntletId(1),
            parent: SessionId(7),
            goal: "g".into(),
            artifact: "a".into(),
            bar: vec!["b".into()],
            builder: "x".into(),
            critic: "y".into(),
            max_rounds: None,
            now_ms: 0,
        };
        let sin_barra = NewGauntlet {
            bar: vec![],
            ..base.clone()
        };
        assert_eq!(
            Gauntlet::new(sin_barra).unwrap_err(),
            GauntletProblem::EmptyBar
        );
        let sin_objetivo = NewGauntlet {
            goal: "  ".into(),
            ..base
        };
        assert_eq!(
            Gauntlet::new(sin_objetivo).unwrap_err(),
            GauntletProblem::Blank("goal")
        );
    }

    #[test]
    fn a_new_gauntlet_starts_building_its_first_round() {
        let g = nuevo(None);
        assert_eq!(g.phase, GauntletPhase::Building);
        assert_eq!(g.round, 1);
        assert!(g.rounds.is_empty());
    }

    #[test]
    fn the_ab_order_is_drawn_from_the_seed_and_recorded_per_round() {
        let mut g = nuevo(None);
        assert_eq!(g.begin_judging(0).unwrap(), AbOrder::ArtifactFirst);
        assert_eq!(g.phase, GauntletPhase::Judging);
        assert_eq!(g.rounds.len(), 1);
        assert_eq!(g.rounds[0].order, AbOrder::ArtifactFirst);
        let mut otro = nuevo(None);
        assert_eq!(otro.begin_judging(1).unwrap(), AbOrder::BarFirst);
    }

    #[test]
    fn judging_cannot_begin_twice_or_from_a_terminal_phase() {
        let mut g = en_juicio(None);
        assert!(matches!(
            g.begin_judging(0),
            Err(GauntletProblem::InvalidTransition { .. })
        ));
        g.cancel().unwrap();
        assert!(g.begin_judging(0).is_err());
    }

    #[test]
    fn a_gap_before_the_round_limit_goes_back_to_the_builder_with_only_the_gap() {
        let mut g = en_juicio(Some(2));
        assert_eq!(g.apply_verdict(brecha()).unwrap(), Next::Rebuild);
        assert_eq!(g.phase, GauntletPhase::Rebuilding);
        assert_eq!(g.round, 2);
        assert_eq!(g.last_gap.as_deref(), Some("los márgenes"));
    }

    #[test]
    fn a_gap_on_the_last_round_stops_and_leaves_the_decision_to_the_person() {
        let mut g = en_juicio(Some(1));
        assert_eq!(g.apply_verdict(brecha()).unwrap(), Next::Stop);
        assert_eq!(g.phase, GauntletPhase::Stopped);
        assert!(g.phase.is_terminal());
    }

    #[test]
    fn the_first_no_gap_asks_for_a_recheck_with_the_order_swapped() {
        let mut g = en_juicio(None);
        let primero = g.rounds[0].order;
        assert_eq!(
            g.apply_verdict(Verdict::NoGap).unwrap(),
            Next::Recheck(primero.swapped())
        );
        assert_eq!(g.phase, GauntletPhase::Judging);
        let ultima = g.rounds.last().unwrap();
        assert!(ultima.bias_check);
        assert_eq!(ultima.order, primero.swapped());
        assert_eq!(g.rounds.len(), 2);
    }

    #[test]
    fn a_coherent_recheck_reaches_final_acceptance_never_completed() {
        let mut g = en_juicio(None);
        g.apply_verdict(Verdict::NoGap).unwrap();
        assert_eq!(g.apply_verdict(Verdict::NoGap).unwrap(), Next::Accept);
        assert_eq!(g.phase, GauntletPhase::AwaitingFinalAcceptance);
        assert!(!g.phase.is_terminal());
    }

    #[test]
    fn a_contradicting_recheck_is_inconclusive() {
        let mut g = en_juicio(None);
        g.apply_verdict(Verdict::NoGap).unwrap();
        assert_eq!(g.apply_verdict(brecha()).unwrap(), Next::Inconclusive);
        assert_eq!(g.phase, GauntletPhase::Inconclusive);
    }

    #[test]
    fn a_malformed_verdict_is_inconclusive_not_approval() {
        let mut g = en_juicio(None);
        assert_eq!(
            g.apply_verdict(Verdict::Malformed).unwrap(),
            Next::Inconclusive
        );
        assert_eq!(g.phase, GauntletPhase::Inconclusive);
    }

    #[test]
    fn verdicts_are_only_accepted_while_judging() {
        let mut g = nuevo(None);
        assert!(matches!(
            g.apply_verdict(Verdict::NoGap),
            Err(GauntletProblem::InvalidTransition { .. })
        ));
    }

    #[test]
    fn the_rebuilt_round_is_judged_again_and_can_then_converge() {
        let mut g = en_juicio(Some(2));
        g.apply_verdict(brecha()).unwrap();
        g.begin_judging(1).unwrap();
        assert_eq!(g.rounds.last().unwrap().round, 2);
        assert!(!g.rounds.last().unwrap().bias_check);
        g.apply_verdict(Verdict::NoGap).unwrap();
        assert_eq!(g.apply_verdict(Verdict::NoGap).unwrap(), Next::Accept);
    }

    #[test]
    fn only_an_explicit_accept_from_awaiting_acceptance_completes() {
        let mut g = en_juicio(None);
        assert!(g.accept().is_err());
        g.apply_verdict(Verdict::NoGap).unwrap();
        g.apply_verdict(Verdict::NoGap).unwrap();
        g.accept().unwrap();
        assert_eq!(g.phase, GauntletPhase::Completed);
        assert!(g.cancel().is_err(), "una fase terminal no se cancela");
    }

    #[test]
    fn terminal_phases_admit_no_further_transition() {
        use GauntletPhase::*;
        for fin in [Completed, Stopped, Inconclusive, Cancelled, Failed] {
            assert!(fin.is_terminal());
            for otra in [Building, Judging, Rebuilding, AwaitingFinalAcceptance] {
                assert!(!fin.can_transition_to(otra), "{fin:?} -> {otra:?}");
            }
        }
    }

    #[test]
    fn phase_names_round_trip_through_their_text_form() {
        use GauntletPhase::*;
        for fase in [
            Building,
            Judging,
            Rebuilding,
            AwaitingFinalAcceptance,
            Completed,
            Stopped,
            Inconclusive,
            Cancelled,
            Failed,
        ] {
            assert_eq!(fase.as_str().parse::<GauntletPhase>().unwrap(), fase);
        }
    }

    // Con `ArtifactFirst` la barra es «B»; con `BarFirst`, «A».
    const ART: AbOrder = AbOrder::ArtifactFirst;

    #[test]
    fn preferring_the_bar_is_a_gap_and_needs_its_description() {
        let informe = "Revisé ambos.\n\n```json\n{\"preferred\":\"B\",\"biggest_gap\":\"los márgenes\"}\n```\n";
        assert_eq!(parse_verdict(informe, ART), brecha());
        let invertido = "{\"preferred\":\"A\",\"biggest_gap\":\"los márgenes\"}";
        assert_eq!(parse_verdict(invertido, AbOrder::BarFirst), brecha());
    }

    #[test]
    fn preferring_the_artifact_or_a_tie_is_no_gap() {
        assert_eq!(parse_verdict("{\"preferred\":\"A\"}", ART), Verdict::NoGap);
        assert_eq!(
            parse_verdict("{\"preferred\":\"B\"}", AbOrder::BarFirst),
            Verdict::NoGap
        );
        assert_eq!(
            parse_verdict("Iguales.\n{\"preferred\":\"tie\"}", ART),
            Verdict::NoGap
        );
    }

    #[test]
    fn the_same_answer_means_the_opposite_when_the_order_is_swapped() {
        let respuesta = "{\"preferred\":\"B\",\"biggest_gap\":\"x\"}";
        assert!(matches!(parse_verdict(respuesta, ART), Verdict::Gap { .. }));
        assert_eq!(parse_verdict(respuesta, AbOrder::BarFirst), Verdict::NoGap);
    }

    #[test]
    fn the_last_answer_wins_when_the_critic_quotes_an_earlier_one() {
        let informe = "Antes dije {\"preferred\":\"B\",\"biggest_gap\":\"x\"} pero ahora:\n{\"preferred\":\"A\"}";
        assert_eq!(parse_verdict(informe, ART), Verdict::NoGap);
    }

    #[test]
    fn nested_objects_after_the_answer_do_not_hide_it() {
        let informe = "{\"preferred\":\"B\",\"biggest_gap\":\"los márgenes\",\"evidence\":{\"file\":\"a.md\"}}";
        assert_eq!(parse_verdict(informe, ART), brecha());
    }

    #[test]
    fn anything_unreadable_is_malformed_never_approval() {
        for informe in [
            "",
            "sin json, solo prosa que dice que todo está bien",
            "{\"preferred\":\"C\"}",
            "{\"preferred\":\"a\"}",
            "{\"biggest_gap\":\"falta la preferencia\"}",
            "{\"preferred\":3}",
            "{\"preferred\":\"B\"}",
            "{\"preferred\":\"B\",\"biggest_gap\":\"   \"}",
            "{\"preferred\":\"A\"",
        ] {
            assert_eq!(
                parse_verdict(informe, ART),
                Verdict::Malformed,
                "{informe:?}"
            );
        }
    }

    #[test]
    fn a_long_gap_is_cut_at_the_cap_on_a_character_boundary() {
        let larga = "ñ".repeat(MAX_GAP_BYTES);
        let informe = format!("{{\"preferred\":\"B\",\"biggest_gap\":\"{larga}\"}}");
        let Verdict::Gap { biggest_gap } = parse_verdict(&informe, ART) else {
            panic!("debía ser una brecha");
        };
        assert!(biggest_gap.len() <= MAX_GAP_BYTES);
        assert!(biggest_gap.chars().all(|c| c == 'ñ'));
    }

    #[test]
    fn only_the_tail_of_a_huge_report_is_scanned() {
        let relleno = "palabra ".repeat(MAX_REPORT_BYTES);
        let al_final = format!("{relleno}\n{{\"preferred\":\"A\"}}");
        assert_eq!(parse_verdict(&al_final, ART), Verdict::NoGap);
        let al_principio = format!("{{\"preferred\":\"A\"}}\n{relleno}");
        assert_eq!(parse_verdict(&al_principio, ART), Verdict::Malformed);
    }

    #[test]
    fn a_gauntlet_survives_a_json_round_trip() {
        let mut g = en_juicio(None);
        g.apply_verdict(Verdict::NoGap).unwrap();
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<Gauntlet>(&json).unwrap(), g);
    }
}
