use clap_noun_verb::Result;
use clap_noun_verb_macros::verb;

use anti_llm_cheat_lsp::wip::{self, GitHubSnapshot, WIP_SNAPSHOT_SCHEMA};
use chrono::Utc;

/// Scan source plus an admitted GitHub activity snapshot and print the WIP closure report.
///
/// `snapshot` is a JSON file using `chatman.wip.github-snapshot/v1`.
/// When `snapshot` is empty, only local source observations are admitted.
#[verb]
pub fn scan(dir: String, snapshot: String, json: bool, ocel: bool) -> Result<()> {
    let target_dir = if dir.is_empty() { ".".to_string() } else { dir };
    let admitted = if snapshot.is_empty() {
        GitHubSnapshot {
            schema_version: WIP_SNAPSHOT_SCHEMA.to_string(),
            observed_at: Utc::now(),
            window_days: 30,
            workspace_repository: None,
            repositories: Vec::new(),
        }
    } else {
        match wip::load_snapshot(&snapshot) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("WIP snapshot REFUSED: {error}");
                std::process::exit(2);
            }
        }
    };

    let report = wip::analyze_path(&admitted, &target_dir);
    if ocel {
        match serde_json::to_string_pretty(&report.to_ocel_value()) {
            Ok(rendered) => println!("{rendered}"),
            Err(error) => {
                eprintln!("WIP OCEL projection failed: {error}");
                std::process::exit(3);
            }
        }
    } else if json {
        match serde_json::to_string_pretty(&report) {
            Ok(rendered) => println!("{rendered}"),
            Err(error) => {
                eprintln!("WIP report serialization failed: {error}");
                std::process::exit(3);
            }
        }
    } else {
        print!("{}", report.to_markdown());
    }
    Ok(())
}

/// Render only the ranked, reversible closure intents from an admitted snapshot.
///
/// This command never executes an intent. Every entry remains `INTENT_ONLY`.
#[verb]
pub fn frontier(dir: String, snapshot: String, json: bool) -> Result<()> {
    let target_dir = if dir.is_empty() { ".".to_string() } else { dir };
    let admitted = match wip::load_snapshot(&snapshot) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("WIP snapshot REFUSED: {error}");
            std::process::exit(2);
        }
    };
    let report = wip::analyze_path(&admitted, &target_dir);

    if json {
        match serde_json::to_string_pretty(&report.closure_frontier) {
            Ok(rendered) => println!("{rendered}"),
            Err(error) => {
                eprintln!("WIP frontier serialization failed: {error}");
                std::process::exit(3);
            }
        }
    } else {
        println!("--- Little's Law Closure Frontier ---");
        for (rank, intent) in report.closure_frontier.iter().enumerate() {
            println!(
                "{:>3}. score={:.3} action={:?} repo={} wip={} authority={} receipt_required={}",
                rank + 1,
                intent.priority_score,
                intent.action,
                intent.repository,
                intent.wip_id,
                intent.authority,
                intent.receipt_required
            );
        }
    }
    Ok(())
}

/// Scan only the local tree for explicit source/dependency/replay WIP markers.
#[verb]
pub fn source(dir: String, json: bool) -> Result<()> {
    let target_dir = if dir.is_empty() { ".".to_string() } else { dir };
    let findings = wip::scan_source_wip(&target_dir);
    if json {
        match serde_json::to_string_pretty(&findings) {
            Ok(rendered) => println!("{rendered}"),
            Err(error) => {
                eprintln!("WIP source serialization failed: {error}");
                std::process::exit(3);
            }
        }
    } else {
        println!("--- Source WIP Findings ---");
        println!("Findings: {}", findings.len());
        for finding in findings {
            println!(
                "  - [{:?}/{:?}] {}:{} {}",
                finding.kind, finding.standing, finding.path, finding.line, finding.message
            );
        }
    }
    Ok(())
}
