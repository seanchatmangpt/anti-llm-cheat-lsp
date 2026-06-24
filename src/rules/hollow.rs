use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

/// Patterns that indicate a hollow implementation masquerading as real code.
/// Each entry: (pattern, diagnostic_code, message, blocking)
///
/// `Ok(None)` and `Ok(Some(vec![]))` are CANDIDATE (non-blocking): legitimate
/// LSP handlers return these for optional or empty-result methods. They require
/// review but do not block the gate. `unimplemented!()`, `todo!()`, and
/// `panic!`-as-stub patterns ARE blocking — they are never valid in admission.
const HOLLOW_PATTERNS: &[(&str, &str, &str, bool)] = &[
    (
        "unimplemented!()",
        "ANTI-LLM-HOLLOW-001",
        "unimplemented!() is a placeholder — hollow by law",
        true,
    ),
    (
        "todo!()",
        "ANTI-LLM-HOLLOW-002",
        "todo!() is a placeholder — hollow by law",
        true,
    ),
    (
        "todo!(\"",
        "ANTI-LLM-HOLLOW-002",
        "todo!() is a placeholder — hollow by law",
        true,
    ),
    (
        "panic!(\"not implemented\")",
        "ANTI-LLM-HOLLOW-003",
        "panic-as-stub detected — hollow by law",
        true,
    ),
    (
        "panic!(\"TODO\")",
        "ANTI-LLM-HOLLOW-003",
        "panic-as-stub detected — hollow by law",
        true,
    ),
    (
        "// TODO:",
        "ANTI-LLM-HOLLOW-004",
        "TODO comment is a placeholder — implement or formally refuse",
        true,
    ),
    (
        "// FIXME:",
        "ANTI-LLM-HOLLOW-005",
        "FIXME comment is a placeholder — implement or formally refuse",
        true,
    ),
    (
        "// PLACEHOLDER",
        "ANTI-LLM-HOLLOW-006",
        "PLACEHOLDER comment — hollow by law",
        true,
    ),
    // CANDIDATE (non-blocking): legitimate LSP handlers return these for optional results.
    (
        "Ok(Some(vec![]))",
        "ANTI-LLM-HOLLOW-007",
        "LSP handler returning empty vec — verify not a hollow stub",
        false,
    ),
    (
        "Ok(None)",
        "ANTI-LLM-HOLLOW-008",
        "LSP handler returning None unconditionally — verify not a stub",
        false,
    ),
    (
        "unreachable!()",
        "ANTI-LLM-HOLLOW-009",
        "unreachable!() in reachable code path — review if genuine",
        true,
    ),
    (
        "Box::new(|| {})",
        "ANTI-LLM-HOLLOW-010",
        "Empty closure boxed as implementation — hollow by law",
        true,
    ),
];

/// Scan Rust source line-by-line for hollow implementation patterns.
///
/// Called from `engine::scan_file` for `.rs` files, producing observations
/// whose `context` field is the raw source line. `hollow::evaluate` then
/// matches `HOLLOW_PATTERNS` against those observations.
pub fn scan_for_hollow(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;

        for (pattern, _code, _msg, _blocking) in HOLLOW_PATTERNS {
            if line.contains(pattern) {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "hollow_smell".to_string(),
                    construct: pattern.to_string(),
                    context: line.to_string(),
                    message: format!("Hollow pattern '{}' detected on line {}", pattern, line_num),
                });
            }
        }
    }

    obs
}

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // Only scan Rust source files — these patterns are Rust-specific.
        if !o.file_path.ends_with(".rs") {
            continue;
        }

        for (pattern, code, msg, blocking) in HOLLOW_PATTERNS {
            if o.context.contains(pattern) || o.construct.contains(pattern) {
                diags.push(AntiLlmDiagnostic {
                    code: code.to_string(),
                    category: "hollow_implementation".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: msg.to_string(),
                    forbidden_implication: format!(
                        "Placeholder({}) => HollowAdmission",
                        pattern.trim()
                    ),
                    blocking: *blocking,
                    required_correction:
                        "Replace with real implementation or formal Refuses-by-law declaration"
                            .to_string(),
                    required_next_proof: "Provide transcript + receipt showing real behavior"
                        .to_string(),
                });
            }
        }
    }

    diags
}
