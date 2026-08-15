use super::model::{ClosureActionKind, ClosureIntent, Standing, WipKind, WipObject};

pub(super) fn build_closure_frontier(wip: &[WipObject]) -> Vec<ClosureIntent> {
    let mut intents = Vec::new();
    for item in wip {
        for action in candidate_actions(item) {
            let cost = estimated_cost(item.kind, action);
            let reduction = 1.0 + item.dependents.len() as f64;
            let score = closure_priority(
                reduction,
                item.dependents.len(),
                item.age_days,
                cost,
                item.standing,
            );
            intents.push(ClosureIntent {
                id: format!("closure:{}:{:?}", item.id, action).to_lowercase(),
                wip_id: item.id.clone(),
                repository: item.repository.clone(),
                action,
                reason: closure_reason(item, action),
                expected_wip_reduction: reduction,
                blocked_dependents: item.dependents.len(),
                age_days: item.age_days,
                estimated_cost: cost,
                priority_score: score,
                authority: "INTENT_ONLY".to_string(),
                receipt_required: true,
            });
        }
    }
    intents
}

fn candidate_actions(item: &WipObject) -> Vec<ClosureActionKind> {
    match item.kind {
        WipKind::Ci => vec![ClosureActionKind::RepairCi],
        WipKind::Dependency | WipKind::CrossRepoBlocker => vec![
            ClosureActionKind::ResolveBlocker,
            ClosureActionKind::MaterializeDependency,
        ],
        WipKind::Evidence => vec![ClosureActionKind::AddEvidence],
        WipKind::Receipt => vec![ClosureActionKind::AddReceipt],
        WipKind::Replay => vec![ClosureActionKind::ReplaceReplayPointer],
        WipKind::Review => vec![
            ClosureActionKind::FinishImplementation,
            ClosureActionKind::RequestReview,
        ],
        WipKind::Merge => vec![ClosureActionKind::Merge],
        WipKind::Release => vec![ClosureActionKind::Release],
        WipKind::OrphanBranch | WipKind::AbandonedExperiment => vec![
            ClosureActionKind::CompareAndSupersede,
            ClosureActionKind::OpenPullRequest,
        ],
        WipKind::StalePr => vec![
            ClosureActionKind::FinishImplementation,
            ClosureActionKind::CompareAndSupersede,
        ],
        WipKind::UnsatisfiedRequirement => vec![
            ClosureActionKind::FinishImplementation,
            ClosureActionKind::CloseSatisfiedIssue,
        ],
        WipKind::GeneratedProjection => vec![ClosureActionKind::FinishImplementation],
        _ => vec![ClosureActionKind::FinishImplementation],
    }
}

fn closure_reason(item: &WipObject, action: ClosureActionKind) -> String {
    let base = match action {
        ClosureActionKind::RepairCi => "repair failed exact-head verification",
        ClosureActionKind::FinishImplementation => "finish admitted incomplete work",
        ClosureActionKind::ResolveBlocker => "resolve blocker before creating additional WIP",
        ClosureActionKind::MaterializeDependency => "restore dependency closure",
        ClosureActionKind::AddEvidence => "close evidence coverage gap",
        ClosureActionKind::AddReceipt => "bind completion to a receipt",
        ClosureActionKind::ReplaceReplayPointer => "make evidence replayable outside one machine",
        ClosureActionKind::RequestReview => "move completed draft work to review",
        ClosureActionKind::Merge => "remove verified merge-ready WIP",
        ClosureActionKind::Release => "convert integrated work into a release",
        ClosureActionKind::OpenPullRequest => "put unmerged branch work on a review/merge path",
        ClosureActionKind::CompareAndSupersede => "classify stale branch work instead of leaving latent WIP",
        ClosureActionKind::CloseSatisfiedIssue => "close a requirement only after observed satisfaction",
        ClosureActionKind::ReclassifyUnsupported => "classify unsupported work without claiming completion",
    };
    format!("{base}; object={}", item.title)
}

fn estimated_cost(kind: WipKind, action: ClosureActionKind) -> f64 {
    match action {
        ClosureActionKind::Merge | ClosureActionKind::CloseSatisfiedIssue => 0.75,
        ClosureActionKind::RequestReview | ClosureActionKind::ReplaceReplayPointer => 1.0,
        ClosureActionKind::AddEvidence | ClosureActionKind::AddReceipt => 1.5,
        ClosureActionKind::RepairCi => 2.0,
        ClosureActionKind::CompareAndSupersede | ClosureActionKind::OpenPullRequest => 2.0,
        ClosureActionKind::ResolveBlocker | ClosureActionKind::MaterializeDependency => 2.5,
        ClosureActionKind::Release => 2.0,
        ClosureActionKind::FinishImplementation => match kind {
            WipKind::Documentation | WipKind::SourceMarker => 1.5,
            WipKind::Test => 2.0,
            WipKind::Code | WipKind::Integration | WipKind::UnsatisfiedRequirement => 3.5,
            _ => 3.0,
        },
        ClosureActionKind::ReclassifyUnsupported => 0.75,
    }
}

pub fn closure_priority(
    expected_wip_reduction: f64,
    blocked_dependents: usize,
    age_days: f64,
    estimated_cost: f64,
    standing: Standing,
) -> f64 {
    let standing_weight = match standing {
        Standing::BuildBroken => 3.0,
        Standing::Blocked => 2.5,
        Standing::PartialAlive => 1.5,
        Standing::Unknown => 1.0,
        Standing::Unsupported | Standing::Refused => 0.5,
        Standing::Alive => 0.0,
    };
    let benefit = expected_wip_reduction
        + 2.0 * blocked_dependents as f64
        + 0.05 * age_days.max(0.0)
        + standing_weight;
    benefit / estimated_cost.max(0.25)
}
