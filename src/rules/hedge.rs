use std::sync::OnceLock;

use regex::Regex;

use crate::{diagnostics::AntiLlmDiagnostic, observations::Observation};

fn hedge_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(in a real implementation|for now,? (we('ll)?|this)|temporary workaround|this is a placeholder|stub implementation|not yet implemented|will be replaced|production implementation would|todo: replace this with a real)",
        )
        .unwrap()
    })
}

/// Returns true if `line` (raw, not trimmed) is inside a comment context.
/// Accepts lines whose trimmed form starts with `//`, or lines inside a
/// `/* … */` block tracked by the caller via `in_block_comment`.
fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//")
}

/// Scan every line for hedge comment patterns.
///
/// Only fires on lines that are single-line comments (`//`) or inside
/// a block comment (`/* … */`). Produces observations with
/// `kind = "hedge_smell"`, `construct` = the matched keyword phrase,
/// and `context` = the full raw line.
pub fn scan_for_hedge(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();
    let mut in_block_comment = false;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim_start();

        // Track block comment entry / exit.
        if trimmed.starts_with("/*") {
            in_block_comment = true;
        }

        let is_comment = in_block_comment || is_comment_line(trimmed);

        if is_comment {
            if let Some(m) = hedge_re().find(line) {
                let matched_phrase = m.as_str().to_string();
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "hedge_smell".to_string(),
                    construct: matched_phrase.clone(),
                    context: line.to_string(),
                    message: format!(
                        "Hedge comment '{}' detected on line {}",
                        matched_phrase, line_num
                    ),
                });
            }
        }

        // Close block comment tracking once `*/` appears on the line.
        if in_block_comment && line.contains("*/") {
            in_block_comment = false;
        }
    }

    obs
}

pub fn evaluate(
    obs: &[Observation],
    config: &crate::config::AntiLlmConfig,
) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        if o.kind != "hedge_smell" {
            continue;
        }
        if config.is_suppression_allowed(&o.file_path) {
            continue;
        }

        diags.push(AntiLlmDiagnostic {
            code: "ANTI-LLM-HEDGE-001".to_string(),
            category: "hedge_implementation".to_string(),
            file_path: o.file_path.clone(),
            line: o.line,
            column: o.column,
            message: format!(
                "Hedge comment detected: '{}' — implementation admitted incomplete via comment",
                o.construct
            ),
            forbidden_implication: format!(
                "HedgeComment({}) => IncompleteImplementation",
                o.construct
            ),
            blocking: true,
            required_correction:
                "Remove the hedge comment and either complete the implementation or use todo!()"
                    .to_string(),
            required_next_proof: "Show the completed implementation with no hedging language"
                .to_string(),
        });
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_in_a_real_implementation() {
        let obs = scan_for_hedge("test.rs", "// In a real implementation, we would do X");
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].kind, "hedge_smell");
    }

    #[test]
    fn detects_for_now_phrase() {
        let obs = scan_for_hedge("test.rs", "// For now, we'll use a stub");
        assert_eq!(obs.len(), 1);
    }

    #[test]
    fn does_not_fire_on_non_comment() {
        let obs = scan_for_hedge("test.rs", r#"let msg = "in a real implementation";"#);
        assert_eq!(obs.len(), 0, "Should not fire on string literals");
    }

    #[test]
    fn does_not_fire_on_clean_code() {
        let obs = scan_for_hedge("test.rs", "pub fn compute(x: u32) -> u32 { x * 2 }");
        assert_eq!(obs.len(), 0);
    }
}
