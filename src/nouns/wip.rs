use anti_llm_cheat_lsp::wip::{
    self, GitHubSnapshot, DEFAULT_PARETO_TARGET_SHARE, WIP_SNAPSHOT_SCHEMA,
};
use chrono::Utc;
use clap_noun_verb::Result;
use clap_noun_verb_macros::verb;

/// Scan source plus an admitted GitHub activity snapshot and print the WIP closure report.
///
/// `snapshot` is a JSON file using `chatman.wip.github-snapshot/v1`.
/// When `snapshot` is empty, only local source observations are admitted.
#[verb]
pub fn scan(dir: String, snapshot: String, json: bool, ocel: bool) -> Result<()> {
    let target_dir = if dir.is_empty() { ".".to_string() } else { dir };
    let admitted = admit_snapshot_or_source_only(&snapshot);

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

/// Render every positive-impact WIP object in deterministic DfCM priority order.
///
/// The historical `pareto` command name is retained for compatibility, but it no longer
/// truncates the lawful option space at an 80% threshold. Every ranked object receives an
/// ERRC lane and every recommended action remains `INTENT_ONLY`.
#[verb]
pub fn pareto(dir: String, snapshot: String, json: bool) -> Result<()> {
    let target_dir = if dir.is_empty() { ".".to_string() } else { dir };
    let admitted = admit_snapshot_or_source_only(&snapshot);
    let report = wip::analyze_path(&admitted, &target_dir);
    let summary = wip::errc_pareto(&report, DEFAULT_PARETO_TARGET_SHARE);

    if json {
        match serde_json::to_string_pretty(&summary) {
            Ok(rendered) => println!("{rendered}"),
            Err(error) => {
                eprintln!("WIP DfCM serialization failed: {error}");
                std::process::exit(3);
            }
        }
    } else {
        println!("--- DfCM Full-Coverage ERRC WIP Frontier ---");
        println!(
            "Ranked {}/{} WIP objects ({:.1}% of observed objects; {:.1}% of positive weighted impact preserved).",
            summary.selected_wip,
            summary.total_wip,
            100.0 * summary.selected_wip_share,
            100.0 * summary.selected_share
        );
        for item in &summary.items {
            let action = item
                .recommended_intent
                .as_ref()
                .map(|intent| format!("{:?}", intent.action))
                .unwrap_or_else(|| "NONE".to_string());
            println!(
                "{:>3}. [{:?}] impact={:.3} share={:.1}% cumulative={:.1}% action={} repo={} wip={}",
                item.rank,
                item.errc_lane,
                item.impact_score,
                100.0 * item.impact_share,
                100.0 * item.cumulative_share,
                action,
                item.repository,
                item.wip_id
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

/// List the Tree-sitter grammars admitted by the source WIP scanner.
#[verb]
pub fn languages(json: bool) -> Result<()> {
    let languages = wip::supported_tree_sitter_languages();
    if json {
        match serde_json::to_string_pretty(languages) {
            Ok(rendered) => println!("{rendered}"),
            Err(error) => {
                eprintln!("WIP language serialization failed: {error}");
                std::process::exit(3);
            }
        }
    } else {
        println!("--- Tree-sitter WIP Languages ---");
        println!("Languages: {}", languages.len());
        for language in languages {
            println!("  - {language}");
        }
    }
    Ok(())
}

fn admit_snapshot_or_source_only(snapshot: &str) -> GitHubSnapshot {
    if snapshot.is_empty() {
        GitHubSnapshot {
            schema_version: WIP_SNAPSHOT_SCHEMA.to_string(),
            observed_at: Utc::now(),
            window_days: 30,
            workspace_repository: None,
            repositories: Vec::new(),
        }
    } else {
        match wip::load_snapshot(snapshot) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("WIP snapshot REFUSED: {error}");
                std::process::exit(2);
            }
        }
    }
}
