use anti_llm_cheat_lsp::wip::{
    analyze, closure_priority, load_snapshot, scan_source_wip, BranchSnapshot, ClosureActionKind,
    GitHubSnapshot, IssueSnapshot, PullRequestSnapshot, RepositorySnapshot, Standing,
    WorkflowRunSnapshot, WipKind, WIP_SNAPSHOT_SCHEMA,
};
use chrono::{DateTime, TimeZone, Utc};
use std::fs;

fn ts(day: u32) -> DateTime<Utc> {
    match Utc.with_ymd_and_hms(2026, 8, day, 0, 0, 0).single() {
        Some(value) => value,
        None => panic!("valid fixture timestamp"),
    }
}

fn snapshot(repositories: Vec<RepositorySnapshot>) -> GitHubSnapshot {
    GitHubSnapshot {
        schema_version: WIP_SNAPSHOT_SCHEMA.to_string(),
        observed_at: ts(15),
        window_days: 30,
        workspace_repository: None,
        repositories,
    }
}

#[test]
fn little_law_uses_observed_closures_only() {
    let mut admitted = snapshot(vec![RepositorySnapshot {
        full_name: "acme/repo".to_string(),
        pull_requests: vec![
            PullRequestSnapshot {
                number: 1,
                state: "open".to_string(),
                created_at: Some(ts(10)),
                updated_at: Some(ts(14)),
                ci_status: "failure".to_string(),
                ..PullRequestSnapshot::default()
            },
            PullRequestSnapshot {
                number: 2,
                state: "closed".to_string(),
                created_at: Some(ts(5)),
                merged_at: Some(ts(10)),
                ..PullRequestSnapshot::default()
            },
        ],
        ..RepositorySnapshot::default()
    }]);
    admitted.window_days = 10;

    let report = analyze(&admitted, Vec::new());
    assert_eq!(report.metrics.wip_l, 1);
    assert_eq!(report.metrics.completed_objects_in_window, 1);
    assert_eq!(report.metrics.throughput_lambda_per_day, Some(0.1));
    assert!(report
        .closure_frontier
        .iter()
        .any(|intent| intent.action == ClosureActionKind::RepairCi));
}

#[test]
fn linked_issue_pr_and_failed_workflow_are_one_wip_object() {
    let admitted = snapshot(vec![RepositorySnapshot {
        full_name: "acme/repo".to_string(),
        pull_requests: vec![PullRequestSnapshot {
            number: 10,
            state: "open".to_string(),
            head_branch: "feature/ten".to_string(),
            ci_status: "failure".to_string(),
            linked_issues: vec![3],
            ..PullRequestSnapshot::default()
        }],
        issues: vec![IssueSnapshot {
            number: 3,
            state: "open".to_string(),
            linked_pr: Some(10),
            ..IssueSnapshot::default()
        }],
        workflow_runs: vec![WorkflowRunSnapshot {
            id: 99,
            conclusion: "failure".to_string(),
            pull_request: Some(10),
            ..WorkflowRunSnapshot::default()
        }],
        ..RepositorySnapshot::default()
    }]);

    let report = analyze(&admitted, Vec::new());
    assert_eq!(report.metrics.wip_l, 1);
    assert_eq!(report.wip[0].kind, WipKind::Ci);
    assert!(report.wip[0]
        .evidence
        .iter()
        .any(|item| item == "linked_issue=3"));
}

#[test]
fn stale_unmerged_branch_becomes_orphan_wip() {
    let admitted = snapshot(vec![RepositorySnapshot {
        full_name: "acme/repo".to_string(),
        branches: vec![BranchSnapshot {
            name: "feature/old".to_string(),
            ahead_by: 3,
            last_activity_at: Some(ts(1)),
            ..BranchSnapshot::default()
        }],
        ..RepositorySnapshot::default()
    }]);

    let report = analyze(&admitted, Vec::new());
    assert_eq!(report.wip[0].kind, WipKind::OrphanBranch);
    assert!(report
        .closure_frontier
        .iter()
        .any(|intent| intent.action == ClosureActionKind::CompareAndSupersede));
}

#[test]
fn closure_priority_prefers_blocking_topology() {
    let isolated = closure_priority(1.0, 0, 10.0, 2.0, Standing::PartialAlive);
    let blocking = closure_priority(1.0, 4, 1.0, 2.0, Standing::Blocked);
    assert!(blocking > isolated);
}

#[test]
fn ocel_projection_never_grants_execution_authority() {
    let admitted = snapshot(vec![RepositorySnapshot {
        full_name: "acme/repo".to_string(),
        issues: vec![IssueSnapshot {
            number: 7,
            state: "open".to_string(),
            ..IssueSnapshot::default()
        }],
        ..RepositorySnapshot::default()
    }]);

    let report = analyze(&admitted, Vec::new());
    assert!(report
        .closure_frontier
        .iter()
        .all(|intent| intent.authority == "INTENT_ONLY"));
    assert!(report
        .closure_frontier
        .iter()
        .all(|intent| intent.receipt_required));

    let ocel = report.to_ocel_value();
    let events = match ocel.get("events").and_then(serde_json::Value::as_array) {
        Some(events) => events,
        None => panic!("OCEL events array missing"),
    };
    assert!(events.iter().any(|event| {
        event.get("type").and_then(serde_json::Value::as_str) == Some("WipObserved")
    }));
    assert!(events.iter().any(|event| {
        event.get("type").and_then(serde_json::Value::as_str)
            == Some("ClosureIntentConstructed")
    }));
}

#[test]
fn generic_status_vocabulary_is_not_source_wip() {
    let dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => panic!("temporary directory failed: {error}"),
    };
    assert!(fs::write(
        dir.path().join("README.md"),
        "Allowed statuses include OPEN, BLOCKED, CANDIDATE, and PARTIAL_ALIVE.\n",
    )
    .is_ok());

    assert!(scan_source_wip(dir.path()).is_empty());
}

#[test]
fn declared_status_is_admitted_as_source_wip() {
    let dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => panic!("temporary directory failed: {error}"),
    };
    assert!(fs::write(dir.path().join("STATUS.md"), "Status: BLOCKED\n").is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].standing, Standing::Blocked);
}

#[test]
fn utf8_source_evidence_is_scanned_without_byte_slicing() {
    let dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => panic!("temporary directory failed: {error}"),
    };
    let line = format!("// TODO {}", "進".repeat(200));
    assert!(fs::write(dir.path().join("lib.rs"), line).is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.ends_with('…'));
}

#[test]
fn missing_snapshot_schema_is_refused() {
    let dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => panic!("temporary directory failed: {error}"),
    };
    let path = dir.path().join("snapshot.json");
    assert!(fs::write(
        &path,
        r#"{"observed_at":"2026-08-15T00:00:00Z","repositories":[]}"#,
    )
    .is_ok());

    assert!(load_snapshot(&path).is_err());
}
