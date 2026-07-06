/// Markdown claims parser — delegates victory vocabulary to `rules::claims`.
///
/// Previously this file maintained its own `VICTORY_PHRASES` list. That list
/// is now the canonical `rules::claims::VICTORY_TERMS` array. This parser is
/// the entry point for `.md` files; it calls `claims::scan_for_victory` so
/// the vocabulary is never duplicated.
use std::sync::OnceLock;

use regex::Regex;

use crate::observations::Observation;
use crate::rules::claims;

/// Standing-index claim vocabulary: `(term, claim_kind)`. Matching is
/// case-insensitive except where noted. `claim_kind` is the bounded label
/// `rules::standing` switches on — it is intentionally coarser than the
/// matched term (e.g. both "production-ready" and "production ready" map to
/// the same `"production-ready"` claim_kind).
///
/// "published" is kept distinct from "publish-ready": the former is a claim
/// that a completed action already happened, the latter is a readiness
/// claim. `praxis-standing.v1` has no `PUBLISHED` status — see
/// `ANTI-LLM-STANDING-003`.
const CLAIM_PATTERNS: &[(&str, &str)] = &[
    ("production-ready", "production-ready"),
    ("production ready", "production-ready"),
    ("pilot-ready", "pilot-ready"),
    ("publish-ready", "publish-ready"),
    ("publication-ready", "publication-ready"),
    ("ALIVE", "alive"),
    ("verified", "verified"),
    ("benchmarked", "benchmarked"),
    ("OCEL-proven", "ocel-proven"),
    ("wasm4pm-proven", "wasm4pm-proven"),
    ("published", "published"),
];

/// Phrases that introduce a scope qualifier, e.g. "PRODUCTION_READY for local
/// deterministic validation" or "PILOT_READY, scoped to the staging tenant".
fn scope_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(?:for|scoped to)\s+([A-Za-z0-9_][A-Za-z0-9_./ -]{2,79})")
            .expect("scope regex compile")
    })
}

/// A backtick-fenced code span, used to capture an artifact identifier
/// (crate/path name) near a claim.
fn code_span_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`([^`\n]{1,120})`").expect("code span regex compile"))
}

fn claim_term_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // "ALIVE" must stay case-sensitive (all-caps) — the lowercase word is
        // common English prose ("keeps the crate alive") and would otherwise
        // flood every doc with false positives. Every other term is
        // case-insensitive per the ticket vocabulary. Flags are scoped with
        // `(?i:...)` per-alternative so ALIVE's case sensitivity isn't
        // clobbered by an earlier alternative's inline `(?i)`.
        let alts = CLAIM_PATTERNS
            .iter()
            .map(|(term, _)| {
                if *term == "ALIVE" {
                    r"\bALIVE\b".to_string()
                } else {
                    format!(r"(?i:\b{}\b)", regex::escape(term))
                }
            })
            .collect::<Vec<_>>()
            .join("|");
        Regex::new(&alts).expect("claim term regex compile")
    })
}

fn claim_kind_for(matched: &str) -> &'static str {
    if matched == "ALIVE" {
        return "alive";
    }
    let lower = matched.to_lowercase();
    CLAIM_PATTERNS
        .iter()
        .find(|(term, _)| term.to_lowercase() == lower)
        .map(|(_, kind)| *kind)
        .unwrap_or("unknown")
}

/// Find a scope phrase in a short window around `line_idx` (same line first,
/// then the next two lines, then the previous two).
fn find_scope(lines: &[&str], line_idx: usize) -> Option<String> {
    let candidates = [
        Some(line_idx),
        line_idx.checked_add(1),
        line_idx.checked_add(2),
        line_idx.checked_sub(1),
        line_idx.checked_sub(2),
    ];
    for cand in candidates.into_iter().flatten() {
        if let Some(line) = lines.get(cand) {
            if let Some(cap) = scope_re().captures(line) {
                return Some(cap[1].trim().trim_end_matches(['.', ',', ';']).to_string());
            }
        }
    }
    None
}

/// Find a nearby artifact identifier: a backtick code span on the claim line,
/// else the nearest code span or markdown heading within a short backward
/// window.
fn find_artifact(lines: &[&str], line_idx: usize) -> Option<String> {
    if let Some(line) = lines.get(line_idx) {
        if let Some(cap) = code_span_re().captures(line) {
            return Some(cap[1].trim().to_string());
        }
    }
    let start = line_idx.saturating_sub(5);
    for idx in (start..line_idx).rev() {
        if let Some(line) = lines.get(idx) {
            if let Some(cap) = code_span_re().captures(line) {
                return Some(cap[1].trim().to_string());
            }
            let trimmed = line.trim_start();
            if let Some(heading) = trimmed.strip_prefix('#') {
                let heading = heading.trim_start_matches('#').trim();
                if !heading.is_empty() {
                    return Some(heading.to_string());
                }
            }
        }
    }
    None
}

/// Scan `content` for standing-index claim vocabulary (`CLAIM_PATTERNS`) and
/// return `standing_claim` observations. Each observation's `context` encodes
/// `claim_kind`, captured `scope` (or `NONE`), captured `artifact` (or
/// `NONE`), and the source line, in the `key=value, ...` shape already used by
/// `parsers::fitness_report`.
fn scan_for_standing_claims(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        for mat in claim_term_re().find_iter(line) {
            let claim_kind = claim_kind_for(mat.as_str());
            let scope = find_scope(&lines, line_idx);
            let artifact = find_artifact(&lines, line_idx);
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_idx + 1,
                column: mat.start() + 1,
                kind: "standing_claim".to_string(),
                construct: claim_kind.to_string(),
                context: format!(
                    "claim_kind={}, scope={}, artifact={}, line={}",
                    claim_kind,
                    scope.as_deref().unwrap_or("NONE"),
                    artifact.as_deref().unwrap_or("NONE"),
                    line.trim()
                ),
                message: format!(
                    "Standing claim '{}' ({}) detected",
                    mat.as_str(),
                    claim_kind
                ),
            });
        }
    }

    obs
}

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
    obs.extend(scan_for_standing_claims(filepath, content));
    obs
}
