/// Markdown claims parser — delegates victory vocabulary to `rules::claims`.
///
/// Previously this file maintained its own `VICTORY_PHRASES` list. That list
/// is now the canonical `rules::claims::VICTORY_TERMS` array. This parser is
/// the entry point for `.md` files; it calls `claims::scan_for_victory` so
/// the vocabulary is never duplicated.
use crate::observations::Observation;
use crate::rules::claims;

pub fn parse_markdown_claims(filepath: &str, content: &str) -> Vec<Observation> {
    // Domain terms are not available at parse time (config is loaded at the
    // directory level). We pass an empty slice here; the claims::evaluate rule
    // applies domain exemptions after all observations are collected.
    claims::scan_for_victory(filepath, content, "markdown_claim", &[])
}
