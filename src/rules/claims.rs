use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;
/// CLAIM-004: Victory language and overclaim detection.
///
/// All victory vocabulary lives here as the single source of truth.
/// `engine.rs` feeds this rule via the raw-smell automaton (which is seeded
/// from `VICTORY_TERMS` below) and the markdown claims parser. The rule then
/// applies domain-term exemptions from per-repo config before emitting
/// diagnostics.
use aho_corasick::{AhoCorasick, MatchKind};
use std::sync::OnceLock;

/// Canonical victory / overclaim terms — the single source of truth.
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

fn victory_ac() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| {
        AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .match_kind(MatchKind::LeftmostFirst)
            .build(VICTORY_TERMS)
            .expect("victory term automaton compile")
    })
}

fn context_ac() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| {
        AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .match_kind(MatchKind::LeftmostFirst)
            .build(VICTORY_CONTEXT_PATTERNS)
            .expect("victory context automaton compile")
    })
}

/// Returns `true` if `term` (lowercased) is covered by a repo-configured
/// domain exemption. Domain terms are canonical vocabulary in the target crate,
/// not overclaims.
fn is_domain_exempt(term: &str, domain_terms: &[String]) -> bool {
    let term_lower = term.to_lowercase();
    domain_terms
        .iter()
        .any(|d| term_lower.contains(d.to_lowercase().as_str()))
}

/// Check whether `construct` (the raw matched text) or `context` (surrounding
/// line) triggers a victory-language violation, respecting domain exemptions.
fn is_victory(construct: &str, context: &str, domain_terms: &[String]) -> bool {
    // Fast path: term-level match via Aho-Corasick
    if victory_ac().is_match(construct) && !is_domain_exempt(construct, domain_terms) {
        return true;
    }
    // Context-level match for multi-word patterns in the surrounding line
    if context_ac().is_match(context) && !is_domain_exempt(context, domain_terms) {
        return true;
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
        for mat in victory_ac().find_iter(line) {
            let term = VICTORY_TERMS[mat.pattern().as_usize()];
            if is_domain_exempt(term, domain_terms) {
                continue;
            }
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_idx + 1,
                column: mat.start() + 1,
                kind: kind.to_string(),
                construct: term.to_string(),
                context: line.trim().to_string(),
                message: format!("Victory/overclaim language '{}' found", term),
            });
        }
        // Context-level matches (surrounding line patterns)
        for mat in context_ac().find_iter(line) {
            let pattern = VICTORY_CONTEXT_PATTERNS[mat.pattern().as_usize()];
            if is_domain_exempt(pattern, domain_terms) {
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
                construct: pattern.to_string(),
                context: line.trim().to_string(),
                message: format!("Victory/overclaim context pattern '{}' found", pattern),
            });
        }
    }

    obs
}
