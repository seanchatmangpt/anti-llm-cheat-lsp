use crate::{diagnostics::AntiLlmDiagnostic, observations::Observation};

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
    ("todo!()", "ANTI-LLM-HOLLOW-002", "todo!() is a placeholder — hollow by law", true),
    ("todo!(\"", "ANTI-LLM-HOLLOW-002", "todo!() is a placeholder — hollow by law", true),
    (
        "panic!(\"not implemented\")",
        "ANTI-LLM-HOLLOW-003",
        "panic-as-stub detected — hollow by law",
        true,
    ),
    ("panic!(\"TODO\")", "ANTI-LLM-HOLLOW-003", "panic-as-stub detected — hollow by law", true),
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
    ("// PLACEHOLDER", "ANTI-LLM-HOLLOW-006", "PLACEHOLDER comment — hollow by law", true),
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
    // HOLLOW-013/014: false-success returns — empty vec/Some masked as valid LSP response (blocking)
    (
        "Ok(vec![])",
        "ANTI-LLM-HOLLOW-013",
        "LSP handler returning Ok(vec![]) — empty success is a false-success stub",
        true,
    ),
    (
        "Ok(Some(vec![]))",
        "ANTI-LLM-HOLLOW-014",
        "LSP handler returning Ok(Some(vec![])) — empty Some is a false-success stub",
        true,
    ),
];

/// TypeScript/JS hollow patterns → ANTI-LLM-HOLLOW-011
const TS_HOLLOW_PATTERNS: &[(&str, &str, bool)] = &[
    (
        "throw new Error('TODO')",
        "TypeScript TODO throw — hollow by law",
        true,
    ),
    (
        "throw new Error(\"TODO\")",
        "TypeScript TODO throw — hollow by law",
        true,
    ),
    (
        "throw new Error('not implemented')",
        "TypeScript not-implemented throw — hollow by law",
        true,
    ),
    (
        "throw new Error(\"not implemented\")",
        "TypeScript not-implemented throw — hollow by law",
        true,
    ),
    (
        "throw new Error('FIXME')",
        "TypeScript FIXME throw — hollow by law",
        true,
    ),
    (
        "throw new Error(\"FIXME\")",
        "TypeScript FIXME throw — hollow by law",
        true,
    ),
    (
        "// TODO: implement",
        "TypeScript TODO comment — hollow by law",
        true,
    ),
    (
        "/* PLACEHOLDER */",
        "TypeScript PLACEHOLDER comment — hollow by law",
        true,
    ),
];

/// Tera template hollow patterns → ANTI-LLM-HOLLOW-012
const TERA_HOLLOW_PATTERNS: &[(&str, &str, bool)] = &[
    ("{# TODO #}", "Tera TODO comment — hollow by law", true),
    ("{# PLACEHOLDER #}", "Tera PLACEHOLDER comment — hollow by law", true),
    ("{# FIXME #}", "Tera FIXME comment — hollow by law", true),
];

fn make_hollow_obs(filepath: &str, line_num: usize, pattern: &str, line: &str) -> Observation {
    Observation {
        file_path: filepath.to_string(),
        start_byte: 0,
        end_byte: 0,
        line: line_num,
        column: 1,
        kind: "hollow_smell".to_string(),
        construct: pattern.to_string(),
        context: line.to_string(),
        message: format!("Hollow pattern '{}' detected on line {}", pattern, line_num),
    }
}

/// Scan TypeScript/JS files for HOLLOW-011 patterns.
pub fn scan_for_hollow_ts(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        for (pattern, _msg, _blocking) in TS_HOLLOW_PATTERNS {
            if line.contains(pattern) {
                obs.push(make_hollow_obs(filepath, line_num, pattern, line));
            }
        }
    }
    obs
}

/// Scan Tera template files for HOLLOW-012 patterns.
pub fn scan_for_hollow_tera(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        for (pattern, _msg, _blocking) in TERA_HOLLOW_PATTERNS {
            if line.contains(pattern) {
                obs.push(make_hollow_obs(filepath, line_num, pattern, line));
            }
        }
    }
    obs
}

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

pub fn evaluate(
    obs: &[Observation],
    config: &crate::config::AntiLlmConfig,
) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        if config.is_suppression_allowed(&o.file_path) {
            continue;
        }

        let is_ts = o.file_path.ends_with(".ts")
            || o.file_path.ends_with(".tsx")
            || o.file_path.ends_with(".js");
        let is_tera = o.file_path.ends_with(".tera");
        let is_rs = o.file_path.ends_with(".rs");

        if is_rs {
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
        } else if is_ts {
            for (pattern, msg, blocking) in TS_HOLLOW_PATTERNS {
                if o.context.contains(pattern) || o.construct.contains(pattern) {
                    diags.push(AntiLlmDiagnostic {
                        code: "ANTI-LLM-HOLLOW-011".to_string(),
                        category: "hollow_implementation".to_string(),
                        file_path: o.file_path.clone(),
                        line: o.line,
                        column: o.column,
                        message: msg.to_string(),
                        forbidden_implication: format!(
                            "TsPlaceholder({}) => HollowAdmission",
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
        } else if is_tera {
            for (pattern, msg, blocking) in TERA_HOLLOW_PATTERNS {
                if o.context.contains(pattern) || o.construct.contains(pattern) {
                    diags.push(AntiLlmDiagnostic {
                        code: "ANTI-LLM-HOLLOW-012".to_string(),
                        category: "hollow_implementation".to_string(),
                        file_path: o.file_path.clone(),
                        line: o.line,
                        column: o.column,
                        message: msg.to_string(),
                        forbidden_implication: format!(
                            "TeraPlaceholder({}) => HollowAdmission",
                            pattern.trim()
                        ),
                        blocking: *blocking,
                        required_correction:
                            "Replace with real Tera template content or formal Refuses-by-law declaration"
                                .to_string(),
                        required_next_proof:
                            "Provide transcript + receipt showing real template behavior"
                                .to_string(),
                    });
                }
            }
        }
    }

    diags
}
