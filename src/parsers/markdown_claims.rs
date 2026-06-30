/// Markdown claims parser — delegates victory vocabulary to `rules::claims`.
///
/// Previously this file maintained its own `VICTORY_PHRASES` list. That list
/// is now the canonical `rules::claims::VICTORY_TERMS` array. This parser is
/// the entry point for `.md` files; it calls `claims::scan_for_victory` so
/// the vocabulary is never duplicated.
use crate::observations::Observation;
use crate::rules::claims;

/// Phrases that indicate a small delta-changelog is being presented as
/// full combinatorial LSP 3.18 spec coverage — CHANGELOG-001.
const CHANGELOG_LAUNDERING_PATTERNS: &[&str] = &[
    "changelog matrix",
    "changelog coverage",
    "15-row changelog",
    "15 row changelog",
    "changelog as spec coverage",
    "changelog covers",
    "changelog demonstrates",
];

/// Scan `content` for changelog-laundering patterns and return observations
/// with kind `changelog_laundering`. Called only on `.md` files.
fn scan_for_changelog_laundering(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let lower = line.to_lowercase();
        for pat in CHANGELOG_LAUNDERING_PATTERNS {
            if lower.contains(pat) {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: line.len(),
                    line: line_idx + 1,
                    column: 1,
                    kind: "changelog_laundering".to_string(),
                    construct: "changelog_laundering".to_string(),
                    context: line.trim().chars().take(120).collect(),
                    message: format!(
                        "Changelog laundering pattern '{}' detected — delta changelog \
                         must not be presented as full spec coverage",
                        pat
                    ),
                });
                break; // one observation per line
            }
        }
    }
    obs
}

pub fn parse_markdown_claims(filepath: &str, content: &str) -> Vec<Observation> {
    // Domain terms are not available at parse time (config is loaded at the
    // directory level). We pass an empty slice here; the claims::evaluate rule
    // applies domain exemptions after all observations are collected.
    let mut obs = claims::scan_for_victory(filepath, content, "markdown_claim", &[]);
    obs.extend(scan_for_changelog_laundering(filepath, content));
    obs
}
