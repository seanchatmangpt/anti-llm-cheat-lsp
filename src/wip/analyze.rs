use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use chrono::{DateTime, Duration, Utc};

use super::{
    frontier::build_closure_frontier,
    model::*,
    source::{finding_id, scan_source_wip},
};

pub fn load_snapshot(path: impl AsRef<Path>) -> Result<GitHubSnapshot, String> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("cannot read snapshot {}: {error}", path.display()))?;
    let snapshot: GitHubSnapshot = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid GitHub snapshot JSON: {error}"))?;
    if snapshot.schema_version != WIP_SNAPSHOT_SCHEMA {
        return Err(format!(
            "unsupported snapshot schema {}; expected {}",
            snapshot.schema_version, WIP_SNAPSHOT_SCHEMA
        ));
    }
    Ok(snapshot)
}

pub fn analyze(snapshot: &GitHubSnapshot, source_findings: Vec<SourceFinding>) -> WipReport {
    let mut wip = Vec::new();
    let mut completion_cycles_days = Vec::new();
    let mut completed_objects = 0usize;
    let window_start = snapshot.observed_at.clone() - Duration::days(i64::from(snapshot.window_days));
    let dependency_index = dependency_dependents(snapshot);

    for repo in &snapshot.repositories {
        let all_pr_numbers: BTreeSet<u64> =
            repo.pull_requests.iter().map(|pr| pr.number).collect();
        let open_pr_numbers: BTreeSet<u64> = repo
            .pull_requests
            .iter()
            .filter(|pr| is_open(&pr.state))
            .map(|pr| pr.number)
            .collect();
        let open_pr_branches: BTreeSet<&str> = repo
            .pull_requests
            .iter()
            .filter(|pr| is_open(&pr.state))
            .map(|pr| pr.head_branch.as_str())
            .collect();

        for pr in &repo.pull_requests {
            if is_open(&pr.state) {
                let activity = pr
                    .updated_at
                    .clone()
                    .or_else(|| latest_commit_at(repo, &pr.head_branch))
                    .or_else(|| pr.created_at.clone());
                let age = age_days(snapshot.observed_at.clone(), activity.clone());
                let (kind, standing) = classify_open_pr(pr, age);
                let id = format!("github:{}/pr/{}", repo.full_name, pr.number);
                let mut evidence = vec![
                    format!("head_branch={}", pr.head_branch),
                    format!("ci={}", pr.ci_status),
                ];
                evidence.extend(
                    pr.linked_issues
                        .iter()
                        .map(|issue| format!("linked_issue={issue}")),
                );
                wip.push(WipObject {
                    id: id.clone(),
                    repository: repo.full_name.clone(),
                    kind,
                    standing,
                    title: if pr.title.is_empty() {
                        format!("PR #{}", pr.number)
                    } else {
                        format!("#{} {}", pr.number, pr.title)
                    },
                    origin: "github.pull_request".to_string(),
                    created_at: pr.created_at.clone(),
                    last_activity_at: activity,
                    age_days: age,
                    blockers: pr.blockers.clone(),
                    dependents: dependents_for(&dependency_index, &id, &repo.full_name),
                    evidence,
                    closure_conditions: pr_closure_conditions(pr),
                });
            } else if let (Some(created), Some(done)) = (
                pr.created_at.clone(),
                pr.merged_at.clone().or_else(|| pr.closed_at.clone()),
            ) {
                admit_completion(
                    snapshot,
                    window_start.clone(),
                    created,
                    done,
                    &mut completed_objects,
                    &mut completion_cycles_days,
                );
            }
        }

        for branch in &repo.branches {
            if branch.name == repo.default_branch
                || branch.merged_into_default
                || branch.ahead_by == 0
                || open_pr_branches.contains(branch.name.as_str())
            {
                continue;
            }
            let activity = branch
                .last_activity_at
                .clone()
                .or_else(|| latest_commit_at(repo, &branch.name));
            let age = age_days(snapshot.observed_at.clone(), activity.clone());
            let stale = age >= DEFAULT_STALE_DAYS as f64;
            let id = format!("github:{}/branch/{}", repo.full_name, branch.name);
            wip.push(WipObject {
                id: id.clone(),
                repository: repo.full_name.clone(),
                kind: if stale {
                    WipKind::OrphanBranch
                } else {
                    WipKind::Code
                },
                standing: Standing::PartialAlive,
                title: format!("unmerged branch {}", branch.name),
                origin: "github.branch".to_string(),
                created_at: None,
                last_activity_at: activity,
                age_days: age,
                blockers: Vec::new(),
                dependents: dependents_for(&dependency_index, &id, &repo.full_name),
                evidence: vec![
                    format!("ahead_by={}", branch.ahead_by),
                    format!("head_sha={}", branch.head_sha),
                ],
                closure_conditions: vec![
                    "compare branch with default branch".to_string(),
                    "either open/finish a PR or classify the branch as superseded".to_string(),
                ],
            });
        }

        for issue in &repo.issues {
            if is_open(&issue.state) {
                if issue
                    .linked_pr
                    .is_some_and(|number| open_pr_numbers.contains(&number))
                {
                    continue;
                }
                let id = format!("github:{}/issue/{}", repo.full_name, issue.number);
                let activity = issue.updated_at.clone().or_else(|| issue.created_at.clone());
                wip.push(WipObject {
                    id: id.clone(),
                    repository: repo.full_name.clone(),
                    kind: WipKind::UnsatisfiedRequirement,
                    standing: if issue.linked_pr.is_some() {
                        Standing::PartialAlive
                    } else {
                        Standing::Unknown
                    },
                    title: if issue.title.is_empty() {
                        format!("Issue #{}", issue.number)
                    } else {
                        format!("#{} {}", issue.number, issue.title)
                    },
                    origin: "github.issue".to_string(),
                    created_at: issue.created_at.clone(),
                    last_activity_at: issue.updated_at.clone(),
                    age_days: age_days(snapshot.observed_at.clone(), activity),
                    blockers: Vec::new(),
                    dependents: dependents_for(&dependency_index, &id, &repo.full_name),
                    evidence: issue
                        .linked_pr
                        .map(|number| vec![format!("linked_pr={number}")])
                        .unwrap_or_default(),
                    closure_conditions: vec![
                        "observe implementation/evidence satisfying the requirement".to_string(),
                        "close only after satisfaction or explicit not-planned classification"
                            .to_string(),
                    ],
                });
            } else if issue
                .linked_pr
                .is_some_and(|number| all_pr_numbers.contains(&number))
            {
                // A linked PR already represents this logical flow unit's exit.
                // Do not inflate lambda by counting its issue projection again.
                continue;
            } else if let (Some(created), Some(done)) =
                (issue.created_at.clone(), issue.closed_at.clone())
            {
                admit_completion(
                    snapshot,
                    window_start.clone(),
                    created,
                    done,
                    &mut completed_objects,
                    &mut completion_cycles_days,
                );
            }
        }

        for release in &repo.releases {
            if release.draft {
                let id = format!("github:{}/release/{}", repo.full_name, release.tag_name);
                wip.push(WipObject {
                    id: id.clone(),
                    repository: repo.full_name.clone(),
                    kind: WipKind::Release,
                    standing: Standing::PartialAlive,
                    title: if release.name.is_empty() {
                        format!("draft release {}", release.tag_name)
                    } else {
                        format!("draft release {} ({})", release.tag_name, release.name)
                    },
                    origin: "github.release".to_string(),
                    created_at: release.created_at.clone(),
                    last_activity_at: release.created_at.clone(),
                    age_days: age_days(snapshot.observed_at.clone(), release.created_at.clone()),
                    blockers: Vec::new(),
                    dependents: dependents_for(&dependency_index, &id, &repo.full_name),
                    evidence: vec![
                        format!("tag={}", release.tag_name),
                        format!("prerelease={}", release.prerelease),
                    ],
                    closure_conditions: vec![
                        "verify release requirements against the exact source identity".to_string(),
                        "publish only through the bounded release path with a receipt".to_string(),
                    ],
                });
            } else if let (Some(created), Some(done)) =
                (release.created_at.clone(), release.published_at.clone())
            {
                admit_completion(
                    snapshot,
                    window_start.clone(),
                    created,
                    done,
                    &mut completed_objects,
                    &mut completion_cycles_days,
                );
            }
        }

        for run in &repo.workflow_runs {
            if !is_failure(&run.conclusion) {
                continue;
            }
            if run
                .pull_request
                .is_some_and(|number| open_pr_numbers.contains(&number))
            {
                continue;
            }
            let id = format!("github:{}/workflow/{}", repo.full_name, run.id);
            let activity = run.updated_at.clone().or_else(|| run.created_at.clone());
            wip.push(WipObject {
                id: id.clone(),
                repository: repo.full_name.clone(),
                kind: WipKind::Ci,
                standing: Standing::BuildBroken,
                title: if run.name.is_empty() {
                    format!("failed workflow {}", run.id)
                } else {
                    format!("failed workflow {} ({})", run.id, run.name)
                },
                origin: "github.workflow_run".to_string(),
                created_at: run.created_at.clone(),
                last_activity_at: run.updated_at.clone(),
                age_days: age_days(snapshot.observed_at.clone(), activity),
                blockers: vec![format!("workflow conclusion={}", run.conclusion)],
                dependents: dependents_for(&dependency_index, &id, &repo.full_name),
                evidence: vec![
                    format!("head_branch={}", run.head_branch),
                    format!("head_sha={}", run.head_sha),
                ],
                closure_conditions: vec![
                    "locate the failed boundary".to_string(),
                    "repair the narrowest cause and rerun the affected verifier".to_string(),
                ],
            });
        }
    }

    wip.extend(collapse_source_findings(snapshot, &source_findings));
    dedup_wip(&mut wip);

    let metrics = little_law_metrics(
        wip.len(),
        snapshot.window_days,
        completed_objects,
        &completion_cycles_days,
    );
    let mut closure_frontier = build_closure_frontier(&wip);
    closure_frontier.sort_by(|left, right| {
        right
            .priority_score
            .partial_cmp(&left.priority_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });

    let status = aggregate_status(&wip);
    WipReport {
        schema_version: WIP_REPORT_SCHEMA.to_string(),
        observed_at: snapshot.observed_at.clone(),
        status,
        metrics,
        wip,
        closure_frontier,
        source_findings,
        notes: vec![
            "GitHub activity is admitted only from the supplied snapshot; this module performs no network access."
                .to_string(),
            "Closure frontier entries are CONSTRUCT intents, not DO authority; external bounded actuation and receipts remain required."
                .to_string(),
            "UNKNOWN is never promoted to ALIVE from absence of findings.".to_string(),
        ],
    }
}

pub fn analyze_path(snapshot: &GitHubSnapshot, root: impl AsRef<Path>) -> WipReport {
    analyze(snapshot, scan_source_wip(root))
}

fn classify_open_pr(pr: &PullRequestSnapshot, age_days: f64) -> (WipKind, Standing) {
    if is_failure(&pr.ci_status) {
        (WipKind::Ci, Standing::BuildBroken)
    } else if !pr.blockers.is_empty() {
        (WipKind::CrossRepoBlocker, Standing::Blocked)
    } else if is_success(&pr.ci_status) {
        (WipKind::Merge, Standing::PartialAlive)
    } else if age_days >= DEFAULT_STALE_DAYS as f64 {
        (WipKind::StalePr, Standing::PartialAlive)
    } else {
        (WipKind::Review, Standing::PartialAlive)
    }
}

fn collapse_source_findings(
    snapshot: &GitHubSnapshot,
    findings: &[SourceFinding],
) -> Vec<WipObject> {
    let repository = snapshot
        .workspace_repository
        .clone()
        .or_else(|| {
            (snapshot.repositories.len() == 1)
                .then(|| snapshot.repositories[0].full_name.clone())
        })
        .unwrap_or_else(|| "local-workspace".to_string());
    let dependency_index = dependency_dependents(snapshot);
    let mut grouped: BTreeMap<(String, WipKind), Vec<&SourceFinding>> = BTreeMap::new();
    for finding in findings {
        grouped
            .entry((finding.path.clone(), finding.kind))
            .or_default()
            .push(finding);
    }

    grouped
        .into_iter()
        .map(|((path, kind), group)| {
            let standing = group
                .iter()
                .map(|finding| finding.standing)
                .max_by_key(|standing| standing_rank(*standing))
                .unwrap_or(Standing::Unknown);
            let markers = group
                .iter()
                .map(|finding| finding.marker.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let evidence = group
                .iter()
                .map(|finding| format!("{}:{}:{}", finding.path, finding.line, finding.marker))
                .collect();
            let representative = group[0];
            let id = finding_id(&path, 0, &format!("{kind:?}"));
            WipObject {
                id: id.clone(),
                repository: repository.clone(),
                kind,
                standing,
                title: format!(
                    "{} source WIP finding(s) in {} [{}]",
                    group.len(),
                    path,
                    markers
                ),
                origin: "source.scan".to_string(),
                created_at: None,
                last_activity_at: None,
                age_days: 0.0,
                blockers: if standing == Standing::Blocked {
                    vec![representative.message.clone()]
                } else {
                    Vec::new()
                },
                dependents: dependents_for(&dependency_index, &id, &repository),
                evidence,
                closure_conditions: source_closure_conditions(representative),
            }
        })
        .collect()
}

fn source_closure_conditions(finding: &SourceFinding) -> Vec<String> {
    match finding.kind {
        WipKind::Dependency => vec![
            "materialize or pin the dependency".to_string(),
            "execute the dependency-closed verifier".to_string(),
        ],
        WipKind::Replay => vec![
            "replace machine-local pointer with repository-relative or content-addressed evidence"
                .to_string(),
        ],
        WipKind::Evidence => vec![
            "produce exact observed transcript/evidence".to_string(),
            "bind the evidence to the admitted subject".to_string(),
        ],
        WipKind::Receipt => vec!["produce and verify the required receipt".to_string()],
        _ => vec![
            "finish or intentionally classify the incomplete work".to_string(),
            "execute the narrowest verifier covering the changed boundary".to_string(),
        ],
    }
}

fn pr_closure_conditions(pr: &PullRequestSnapshot) -> Vec<String> {
    if is_failure(&pr.ci_status) {
        return vec![
            "inspect the exact-head failing check".to_string(),
            "repair the failed transition and rerun the verifier".to_string(),
        ];
    }
    if !pr.blockers.is_empty() {
        return vec!["resolve admitted blockers".to_string()];
    }
    if pr.draft {
        return vec![
            "finish implementation and evidence".to_string(),
            "mark ready only after exact-head verification".to_string(),
        ];
    }
    if is_success(&pr.ci_status) {
        return vec!["verify merge eligibility at the exact head".to_string()];
    }
    vec!["obtain exact-head verification evidence".to_string()]
}

fn little_law_metrics(
    wip_l: usize,
    window_days: u32,
    completed_objects: usize,
    completion_cycles_days: &[f64],
) -> LittleLawMetrics {
    let throughput = (window_days > 0 && completed_objects > 0)
        .then(|| completed_objects as f64 / f64::from(window_days));
    let mean_cycle = (!completion_cycles_days.is_empty()).then(|| {
        completion_cycles_days.iter().sum::<f64>() / completion_cycles_days.len() as f64
    });
    let projected = throughput.and_then(|lambda| (lambda > 0.0).then(|| wip_l as f64 / lambda));
    LittleLawMetrics {
        wip_l,
        throughput_lambda_per_day: throughput,
        mean_cycle_time_days: mean_cycle,
        projected_wip_time_days: projected,
        observation_window_days: window_days,
        completed_objects_in_window: completed_objects,
    }
}

fn admit_completion(
    snapshot: &GitHubSnapshot,
    window_start: DateTime<Utc>,
    created: DateTime<Utc>,
    done: DateTime<Utc>,
    completed_objects: &mut usize,
    completion_cycles_days: &mut Vec<f64>,
) {
    if done >= window_start && done <= snapshot.observed_at {
        *completed_objects += 1;
        completion_cycles_days.push(duration_days(done - created));
    }
}

fn aggregate_status(wip: &[WipObject]) -> Standing {
    if wip
        .iter()
        .any(|item| item.standing == Standing::BuildBroken)
    {
        Standing::BuildBroken
    } else if wip.iter().any(|item| item.standing == Standing::Blocked) {
        Standing::Blocked
    } else if wip.iter().any(|item| item.standing == Standing::Refused) {
        Standing::Refused
    } else if wip
        .iter()
        .any(|item| item.standing == Standing::Unsupported)
    {
        Standing::Unsupported
    } else if wip.is_empty() {
        Standing::Unknown
    } else {
        Standing::PartialAlive
    }
}

fn latest_commit_at(repo: &RepositorySnapshot, branch: &str) -> Option<DateTime<Utc>> {
    repo.commits
        .iter()
        .filter(|commit| commit.branch == branch)
        .filter_map(|commit| commit.committed_at.clone())
        .max()
}

fn dependency_dependents(snapshot: &GitHubSnapshot) -> BTreeMap<String, Vec<String>> {
    let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for repo in &snapshot.repositories {
        for edge in &repo.dependency_edges {
            if edge.blocking {
                index
                    .entry(edge.to.clone())
                    .or_default()
                    .push(edge.from.clone());
            }
        }
    }
    index
}

fn dependents_for(
    index: &BTreeMap<String, Vec<String>>,
    object_id: &str,
    repository: &str,
) -> Vec<String> {
    let mut dependents = BTreeSet::new();
    if let Some(items) = index.get(object_id) {
        dependents.extend(items.iter().cloned());
    }
    if let Some(items) = index.get(repository) {
        dependents.extend(items.iter().cloned());
    }
    dependents.into_iter().collect()
}

fn dedup_wip(wip: &mut Vec<WipObject>) {
    let mut by_id = BTreeMap::new();
    for item in wip.drain(..) {
        by_id.entry(item.id.clone()).or_insert(item);
    }
    *wip = by_id.into_values().collect();
}

fn standing_rank(standing: Standing) -> u8 {
    match standing {
        Standing::Unknown => 0,
        Standing::Alive => 1,
        Standing::PartialAlive => 2,
        Standing::Unsupported | Standing::Refused => 3,
        Standing::Blocked => 4,
        Standing::BuildBroken => 5,
    }
}

fn age_days(observed_at: DateTime<Utc>, from: Option<DateTime<Utc>>) -> f64 {
    from.map(|timestamp| duration_days(observed_at - timestamp))
        .unwrap_or(0.0)
        .max(0.0)
}

fn duration_days(duration: chrono::Duration) -> f64 {
    duration.num_seconds() as f64 / 86_400.0
}

fn is_open(state: &str) -> bool {
    state.eq_ignore_ascii_case("open")
}

fn is_failure(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "failure" | "failed" | "error" | "cancelled" | "timed_out" | "action_required"
    )
}

fn is_success(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "success" | "successful" | "passed"
    )
}
