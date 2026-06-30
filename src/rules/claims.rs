use std::sync::OnceLock;

/// CLAIM-004: Victory language and overclaim detection.
///
/// All victory vocabulary lives here as the single source of truth for this server.
/// `engine.rs` feeds this rule via the raw-smell automaton (seeded from
/// `VICTORY_TERMS` below) and the markdown claims parser. The rule then
/// applies domain-term exemptions from per-repo config before emitting
/// diagnostics.
use regex::Regex;

use crate::{diagnostics::AntiLlmDiagnostic, observations::Observation};

/// Canonical victory / overclaim terms — single source of truth for anti-llm-cheat-lsp.
///
/// All entries are lowercased; matching is case-insensitive. Add new terms
/// here; they are automatically picked up by the raw-smell automaton in
/// `engine.rs` and by the markdown claims parser.
///
/// To suppress a term for a specific repo (e.g. a typestate crate where
/// "fully admitted" is canonical vocabulary), add it to `anti-llm.toml`:
/// ```toml
/// [claim]
/// domain_terms = ["fully admitted"]
/// ```
pub const VICTORY_TERMS: &[&str] = &[
    // Explicit victory language
    "victory confirmed",
    "victory audit",
    "victory",
    "done",
    // Gap / issue dismissal
    "all gaps resolved",
    "all clean",
    "no issues",
    "everything passes",
    // Overclaims of proof
    "successfully proven",
    "guaranteed",
    "impossible to fake",
    "solved",
    // Route / admission overclaims
    "fully admitted",
    "path is clear",
    "routing to packplan",
    // CLAIM-007: Victory language paraphrases — common LLM circumlocutions
    "implementation is complete",
    "fully implemented",
    "all passing",
    "no remaining issues",
    "everything is resolved",
    "working correctly",
    "no blockers",
    "work is complete",
    "all tests pass",
    "no issues found",
    "all checks pass",
    "all requirements met",
    // GAP-004: additional paraphrases common in LLM output
    "tests are passing",
    "everything works",
    "fully working",
    "complete and working",
    "ready to ship",
    "ready for review",
    "all good",
    "looks good",
    "should be good",
];

/// Context patterns (checked against the surrounding line, not just the
/// matched construct). These catch phrasing that evades term-exact matching.
const VICTORY_CONTEXT_PATTERNS: &[&str] = &[
    "no gaps found",
    "all systems functional",
    "audit complete",
    "zero violations",
    "zero diagnostics",
];

fn victory_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let terms = VICTORY_TERMS.iter().map(|t| regex::escape(t)).collect::<Vec<_>>().join("|");
        Regex::new(&format!(r"(?i)\b({})\b", terms)).expect("victory regex compile")
    })
}

fn context_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let terms =
            VICTORY_CONTEXT_PATTERNS.iter().map(|t| regex::escape(t)).collect::<Vec<_>>().join("|");
        Regex::new(&format!(r"(?i)\b({})\b", terms)).expect("context regex compile")
    })
}

/// Returns `true` if `term` (lowercased) is covered by a repo-configured
/// domain exemption. Domain terms are canonical vocabulary in the target crate,
/// not overclaims.
fn is_domain_exempt(term: &str, domain_terms: &[String]) -> bool {
    let term_lower = term.to_lowercase();
    domain_terms.iter().any(|d| term_lower.contains(d.to_lowercase().as_str()))
}

/// Check whether `construct` (the raw matched text) or `context` (surrounding
/// line) triggers a victory-language violation, respecting domain exemptions.
fn is_victory(construct: &str, context: &str, domain_terms: &[String]) -> bool {
    // Fast path: term-level match via Regex (with word boundaries)
    if let Some(mat) = victory_re().find(construct) {
        if !is_domain_exempt(mat.as_str(), domain_terms) {
            return true;
        }
    }
    // Context-level match for multi-word patterns in the surrounding line
    if let Some(mat) = context_re().find(context) {
        if !is_domain_exempt(mat.as_str(), domain_terms) {
            return true;
        }
    }
    false
}

pub fn evaluate(
    obs: &[Observation],
    domain_terms: &[String],
    _failset_nonempty: bool,
) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // CHANGELOG-001: delta changelog presented as full spec coverage.
        if o.kind == "changelog_laundering" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-CHANGELOG-001".to_string(),
                category: "claim".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!(
                    "Changelog laundering detected: '{}'. A delta changelog is not \
                     combinatorial spec coverage.",
                    o.context.chars().take(80).collect::<String>()
                ),
                forbidden_implication:
                    "ChangelogCoverage => SpecCoverage(LSP 3.18)".to_string(),
                blocking: true,
                required_correction:
                    "Remove claim that changelog matrix constitutes full LSP 3.18 \
                     combinatorial coverage. Implement a spec extractor."
                        .to_string(),
                required_next_proof:
                    "Changelog and spec coverage are tracked separately; \
                     no CHANGELOG-001 diagnostic fires."
                        .to_string(),
            });
            continue;
        }

        if is_victory(&o.construct, &o.context, domain_terms) {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-CLAIM-004".to_string(),
                category: "claim".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!(
                    "Victory/overclaim language detected: '{}'. Bounded status vocabulary required.",
                    o.construct
                ),
                forbidden_implication: "StatusWord(ADMITTED) => Admitted".to_string(),
                blocking: true,
                required_correction: "Replace with bounded status vocabulary (e.g. \
                    REPORTED_ADMITTED_BY_DOGFOOD, CANDIDATE). If this is a domain term \
                    (e.g. typestate vocabulary), add it to anti-llm.toml [claim] domain_terms."
                    .to_string(),
                required_next_proof: "Run admissibility scan; confirm zero CLAIM-004 diagnostics."
                    .to_string(),
            });
        }
    }

    diags
}

/// Scan `content` for victory language and return observations.
///
/// This is called by the markdown claims parser and any other parser that
/// needs to check arbitrary text. It replaces the per-parser vocabulary lists
/// that previously duplicated `VICTORY_TERMS`.
pub fn scan_for_victory(
    filepath: &str,
    content: &str,
    kind: &str,
    domain_terms: &[String],
) -> Vec<Observation> {
    use crate::observations::Observation;
    let mut obs = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        // Term-level matches
        for mat in victory_re().find_iter(line) {
            let term = mat.as_str().to_lowercase();
            if is_domain_exempt(&term, domain_terms) {
                continue;
            }
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_idx + 1,
                column: mat.start() + 1,
                kind: kind.to_string(),
                construct: term.clone(),
                context: line.trim().to_string(),
                message: format!("Victory/overclaim language '{}' found", term),
            });
        }
        // Context-level matches (surrounding line patterns)
        for mat in context_re().find_iter(line) {
            let pattern = mat.as_str().to_lowercase();
            if is_domain_exempt(&pattern, domain_terms) {
                continue;
            }
            // Avoid double-emitting if already captured by term scan
            if obs.last().map(|o: &Observation| o.line) == Some(line_idx + 1) {
                continue;
            }
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_idx + 1,
                column: mat.start() + 1,
                kind: kind.to_string(),
                construct: pattern.clone(),
                context: line.trim().to_string(),
                message: format!("Victory/overclaim context pattern '{}' found", pattern),
            });
        }
    }

    obs
}
