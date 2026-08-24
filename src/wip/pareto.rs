use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::model::{ClosureIntent, Standing, WipKind, WipObject, WipReport};

/// DfCM never deletes lawful reversible WIP options merely because a focus threshold was met.
/// The legacy `pareto` surface is retained as a compatibility name, but its admitted target is
/// complete weighted-impact closure.
pub const DEFAULT_PARETO_TARGET_SHARE: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrcLane {
    Eliminate,
    Reduce,
    Raise,
    Create,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoWip {
    pub rank: usize,
    pub wip_id: String,
    pub repository: String,
    pub kind: WipKind,
    pub standing: Standing,
    pub errc_lane: ErrcLane,
    pub impact_score: f64,
    pub impact_share: f64,
    pub cumulative_share: f64,
    pub recommended_intent: Option<ClosureIntent>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoSummary {
    /// Compatibility field. Under DfCM this is always 1.0 for admitted execution.
    pub target_share: f64,
    pub total_impact: f64,
    pub selected_impact: f64,
    pub selected_share: f64,
    pub selected_wip: usize,
    pub total_wip: usize,
    pub selected_wip_share: f64,
    pub items: Vec<ParetoWip>,
}

/// Rank every positive-impact logical WIP object without truncating the lawful option space.
///
/// `target_share` is accepted for API compatibility only. DfCM admission raises it to complete
/// closure: every observed positive-impact WIP object remains represented exactly once. The
/// impact score is therefore an ordering heuristic, never a selection/deletion authority.
pub fn errc_pareto(report: &WipReport, target_share: f64) -> ParetoSummary {
    let target_share = normalize_target(target_share);
    let mut scored = report
        .wip
        .iter()
        .map(|item| (item, wip_impact(item)))
        .filter(|(_, score)| *score > 0.0)
        .collect::<Vec<_>>();

    scored.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .partial_cmp(left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });

    let total_impact = scored.iter().map(|(_, score)| score).sum::<f64>();
    if scored.is_empty() || total_impact <= f64::EPSILON {
        return ParetoSummary {
            target_share,
            total_impact: 0.0,
            selected_impact: 0.0,
            selected_share: 0.0,
            selected_wip: 0,
            total_wip: report.wip.len(),
            selected_wip_share: 0.0,
            items: Vec::new(),
        };
    }

    let best_intents = best_intent_by_wip(&report.closure_frontier);
    let mut selected_impact = 0.0;
    let mut items = Vec::with_capacity(scored.len());

    for (item, impact_score) in scored {
        selected_impact += impact_score;
        let lane = errc_lane(item.kind);
        items.push(ParetoWip {
            rank: items.len() + 1,
            wip_id: item.id.clone(),
            repository: item.repository.clone(),
            kind: item.kind,
            standing: item.standing,
            errc_lane: lane,
            impact_score,
            impact_share: impact_score / total_impact,
            cumulative_share: selected_impact / total_impact,
            recommended_intent: best_intents.get(&item.id).cloned(),
            rationale: errc_rationale(lane, item),
        });
    }

    let selected_wip = items.len();
    let total_wip = report.wip.len();
    ParetoSummary {
        target_share,
        total_impact,
        selected_impact,
        selected_share: selected_impact / total_impact,
        selected_wip,
        total_wip,
        selected_wip_share: if total_wip == 0 {
            0.0
        } else {
            selected_wip as f64 / total_wip as f64
        },
        items,
    }
}

/// Weighted closure impact used only for prioritization. It is not proof of value
/// and never grants execution authority.
pub fn wip_impact(item: &WipObject) -> f64 {
    if item.standing == Standing::Alive {
        return 0.0;
    }
    let standing_weight = match item.standing {
        Standing::BuildBroken => 8.0,
        Standing::Blocked => 7.0,
        Standing::PartialAlive => 4.0,
        Standing::Unknown => 3.0,
        Standing::Unsupported => 2.0,
        Standing::Refused => 1.0,
        Standing::Alive => 0.0,
    };
    let age_weight = 0.05 * item.age_days.clamp(0.0, 90.0);
    1.0 + standing_weight
        + 3.0 * item.dependents.len() as f64
        + 1.5 * item.blockers.len() as f64
        + age_weight
}

pub const fn errc_lane(kind: WipKind) -> ErrcLane {
    match kind {
        WipKind::OrphanBranch
        | WipKind::AbandonedExperiment
        | WipKind::DuplicateImplementation
        | WipKind::GeneratedProjection => ErrcLane::Eliminate,

        WipKind::Code
        | WipKind::Test
        | WipKind::Review
        | WipKind::Merge
        | WipKind::Documentation
        | WipKind::StalePr
        | WipKind::UnsatisfiedRequirement
        | WipKind::SourceMarker => ErrcLane::Reduce,

        WipKind::Ci | WipKind::Dependency | WipKind::CrossRepoBlocker | WipKind::Replay => {
            ErrcLane::Raise
        }

        WipKind::Integration | WipKind::Evidence | WipKind::Receipt | WipKind::Release => {
            ErrcLane::Create
        }
    }
}

fn best_intent_by_wip(intents: &[ClosureIntent]) -> BTreeMap<String, ClosureIntent> {
    let mut best = BTreeMap::new();
    for intent in intents {
        match best.get(&intent.wip_id) {
            Some(current)
                if current.priority_score > intent.priority_score
                    || (current.priority_score == intent.priority_score
                        && current.id <= intent.id) => {}
            _ => {
                best.insert(intent.wip_id.clone(), intent.clone());
            }
        }
    }
    best
}

fn normalize_target(_target_share: f64) -> f64 {
    DEFAULT_PARETO_TARGET_SHARE
}

fn errc_rationale(lane: ErrcLane, item: &WipObject) -> String {
    let policy = match lane {
        ErrcLane::Eliminate => {
            "eliminate redundant or latent flow instead of spending capacity finishing unnecessary work"
        }
        ErrcLane::Reduce => {
            "reduce scope to the smallest verifiable closure unit before accepting more WIP"
        }
        ErrcLane::Raise => {
            "raise verification, dependency, or replay quality so blocked flow can close lawfully"
        }
        ErrcLane::Create => {
            "create only the missing closure artifact required to retire existing WIP"
        }
    };
    format!("{policy}; object={}", item.title)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::wip::{LittleLawMetrics, WIP_REPORT_SCHEMA};

    fn object(
        id: &str,
        kind: WipKind,
        standing: Standing,
        dependents: usize,
        blockers: usize,
    ) -> WipObject {
        WipObject {
            id: id.to_string(),
            repository: "owner/repo".to_string(),
            kind,
            standing,
            title: id.to_string(),
            origin: "test".to_string(),
            created_at: None,
            last_activity_at: None,
            age_days: 0.0,
            blockers: (0..blockers).map(|n| format!("blocker-{n}")).collect(),
            dependents: (0..dependents).map(|n| format!("dependent-{n}")).collect(),
            evidence: Vec::new(),
            closure_conditions: Vec::new(),
        }
    }

    fn report(wip: Vec<WipObject>) -> WipReport {
        WipReport {
            schema_version: WIP_REPORT_SCHEMA.to_string(),
            observed_at: Utc::now(),
            status: Standing::PartialAlive,
            metrics: LittleLawMetrics::default(),
            wip,
            closure_frontier: Vec::new(),
            source_findings: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn dfcm_preserves_complete_ranked_positive_impact_frontier() {
        let report = report(vec![
            object("critical", WipKind::Ci, Standing::BuildBroken, 4, 2),
            object("medium", WipKind::Code, Standing::PartialAlive, 1, 0),
            object("small", WipKind::SourceMarker, Standing::Unknown, 0, 0),
        ]);
        let summary = errc_pareto(&report, 0.80);
        assert_eq!(summary.items[0].wip_id, "critical");
        assert_eq!(summary.target_share, 1.0);
        assert_eq!(summary.selected_share, 1.0);
        assert_eq!(summary.selected_wip, summary.total_wip);
        assert_eq!(summary.items.len(), 3);
    }

    #[test]
    fn alive_objects_are_observed_but_not_prioritized() {
        let report = report(vec![
            object("done", WipKind::Code, Standing::Alive, 10, 10),
            object("open", WipKind::Code, Standing::Unknown, 0, 0),
        ]);
        let summary = errc_pareto(&report, 0.80);
        assert_eq!(summary.total_wip, 2);
        assert_eq!(summary.selected_wip, 1);
        assert_eq!(summary.items[0].wip_id, "open");
        assert_eq!(summary.selected_share, 1.0);
    }

    #[test]
    fn errc_mapping_preserves_create_as_closure_only() {
        assert_eq!(errc_lane(WipKind::DuplicateImplementation), ErrcLane::Eliminate);
        assert_eq!(errc_lane(WipKind::Code), ErrcLane::Reduce);
        assert_eq!(errc_lane(WipKind::Dependency), ErrcLane::Raise);
        assert_eq!(errc_lane(WipKind::Receipt), ErrcLane::Create);
    }

    #[test]
    fn alive_objects_have_zero_prioritization_impact() {
        let item = object("done", WipKind::Code, Standing::Alive, 10, 10);
        assert_eq!(wip_impact(&item), 0.0);
    }
}
