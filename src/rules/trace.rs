use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

fn is_test_path(path: &str) -> bool {
    path.contains("tests/")
        || path.ends_with("_test.rs")
        || path.contains("/test/")
        || path.contains("fixtures/")
}

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        match o.construct.as_str() {
            // TRACE-001: inference_trace.push with constant string literal
            "trace_constant_push" if !is_test_path(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-TRACE-001".to_string(),
                    category: "trace".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "inference_trace.push() with constant string literal in '{}' — fabricated trace evidence.",
                        o.context
                    ),
                    forbidden_implication: "TraceEntry => ComputedEvidence".to_string(),
                    blocking: true,
                    required_correction: "Replace constant string trace entries with dynamically constructed messages that include computed values (e.g., rule IDs, confidence scores, matched predicates). Static strings prove nothing.".to_string(),
                    required_next_proof: "Trace entries include computed values; step count equals algorithm-derived structural count (e.g., rule firings, decomposition depth).".to_string(),
                });
            }

            // TRACE-002: hardcoded trace length assertion
            "trace_len_magic_assert" if !is_test_path(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-TRACE-002".to_string(),
                    category: "trace".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "assert!(trace.len() == N) with literal integer in '{}' — hardcoded trace count.",
                        o.context
                    ),
                    forbidden_implication: "LenAssertion => DynamicTrace".to_string(),
                    blocking: true,
                    required_correction: "Derive expected step count from algorithm structure (e.g., rule count, composition table size) rather than embedding a magic literal. The count must vary with algorithm parameters.".to_string(),
                    required_next_proof: "Expected step count is computed from the input; hidden oracle with different parameters produces different step count.".to_string(),
                });
            }

            // TRACE-003: format!() with no {} placeholders in trace push
            "trace_static_format" if !is_test_path(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-TRACE-003".to_string(),
                    category: "trace".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "format!() with no {{}} interpolation in trace push in '{}' — static trace masquerading as computed.",
                        o.context
                    ),
                    forbidden_implication: "FormatCall => DynamicContent".to_string(),
                    blocking: true,
                    required_correction: "format!() calls in trace entries must inject computed values via {} placeholders. A format! with no interpolation is a string literal with extra ceremony.".to_string(),
                    required_next_proof: "All trace format strings contain at least one {} with a computed value (rule id, score, predicate, etc.).".to_string(),
                });
            }

            // TRACE-004: uniform trace push across match arms
            "trace_uniform_arms" if !is_test_path(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-TRACE-004".to_string(),
                    category: "trace".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "Identical trace entries across multiple match arms in '{}' — trace does not discriminate between cases.",
                        o.context
                    ),
                    forbidden_implication: "UniformTrace => CaseDiscrimination".to_string(),
                    blocking: true,
                    required_correction: "Each match arm must produce a trace entry that reflects the specific case matched (rule applied, conclusion drawn, etc.). Identical entries across arms mean the trace carries no algorithmic information.".to_string(),
                    required_next_proof: "Trace entries vary across match arms; inspector can reconstruct the execution path from the trace alone.".to_string(),
                });
            }

            _ => {}
        }
    }

    diags
}
