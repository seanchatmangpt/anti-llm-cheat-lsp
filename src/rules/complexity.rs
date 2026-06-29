use crate::{diagnostics::AntiLlmDiagnostic, observations::Observation};

fn is_breed_src(path: &str) -> bool {
    path.contains("breeds/") || path.contains("src/breeds")
}

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        if o.kind != "fn_metric" && o.kind != "ast_node" {
            continue;
        }

        match o.construct.as_str() {
            // METRIC-001: fat function
            "fn_too_long" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-METRIC-001".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "LongFn => SingleResponsibility".to_string(),
                    blocking: false,
                    required_correction: "Decompose function into named sub-operations; no breed run() should exceed 80 LOC.".to_string(),
                    required_next_proof: "Verify decomposed functions each have a single clear purpose.".to_string(),
                });
            }

            // METRIC-002: high cyclomatic
            "fn_high_cyclomatic" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-METRIC-002".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "HighBranch => CleanAlgorithm".to_string(),
                    blocking: false,
                    required_correction: "Reduce conditional branching; extract sub-cases into helper functions with clear algorithmic semantics.".to_string(),
                    required_next_proof: "Cyclomatic complexity ≤ 10 after refactor; run cargo test to verify behavior preserved.".to_string(),
                });
            }

            // METRIC-003: deep nesting
            "fn_deep_nesting" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-METRIC-003".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "DeepNest => ReadableLogic".to_string(),
                    blocking: false,
                    required_correction: "Flatten nesting via early returns, guard clauses, or extracted helpers.".to_string(),
                    required_next_proof: "Max nesting depth ≤ 4; no oracle bypass hidden in deeply nested else branches.".to_string(),
                });
            }

            // METRIC-004: literal-dense function (BLOCKING — oracle lookup table)
            "fn_literal_dense" if is_breed_src(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-METRIC-004".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "LiteralTable => ComputedResult".to_string(),
                    blocking: true,
                    required_correction: "Remove inline literal constants from breed implementation. All outputs must be computed from algorithm execution, not looked up from embedded tables.".to_string(),
                    required_next_proof: "Run hidden oracle test to confirm algorithm produces correct output via computation, not literal retrieval.".to_string(),
                });
            }

            // METRIC-005: large match dispatch (BLOCKING — array dispatch oracle)
            "fn_large_match" if is_breed_src(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-METRIC-005".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "DispatchTable => AlgorithmExecution".to_string(),
                    blocking: true,
                    required_correction: "A match with >20 arms in a breed implementation is a dispatch oracle. Replace with algorithmic computation over the match parameter.".to_string(),
                    required_next_proof: "Hidden oracle test with inputs NOT in the original match arms must still produce correct output.".to_string(),
                });
            }

            // METRIC-006: deep closure chain
            "fn_deep_closures" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-METRIC-006".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "ClosureChain => TransparentAlgorithm".to_string(),
                    blocking: false,
                    required_correction: "Extract nested closures into named functions for auditability. Algorithm logic must not be hidden in closure nesting depth > 3.".to_string(),
                    required_next_proof: "Refactored closures each have a named, testable identity.".to_string(),
                });
            }

            // HALSTEAD-001: low volume in core fn
            "halstead_low_volume" if is_breed_src(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-HALSTEAD-001".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "LowHalsteadVolume(run) => RealAlgorithm".to_string(),
                    blocking: false,
                    required_correction: "A run/compute function with Halstead vocabulary < 10 is a memorization stub. Implement the full algorithm with diverse operators and operands.".to_string(),
                    required_next_proof: "Halstead vocabulary ≥ 10 after algorithm implementation; oracle test passes.".to_string(),
                });
            }

            // HALSTEAD-002: low operand vocabulary
            "halstead_low_vocabulary" if is_breed_src(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-HALSTEAD-002".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "FewOperands => ComputedOutput".to_string(),
                    blocking: false,
                    required_correction: "Critically low operand vocabulary indicates a memorization stub operating on ≤4 distinct values. Real algorithm implementations have rich operand diversity.".to_string(),
                    required_next_proof: "n2 (distinct operands) ≥ 5 in the refactored implementation.".to_string(),
                });
            }

            // HALSTEAD-003: operator dominated
            "halstead_operator_dominance" if is_breed_src(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-HALSTEAD-003".to_string(),
                    category: "complexity".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "OperatorDominance => DataProcessing".to_string(),
                    blocking: false,
                    required_correction: "Operator-dominated functions are control-flow-only with minimal data — characteristic of oracle passthrough. Introduce real data transformation.".to_string(),
                    required_next_proof: "n2/n1 ratio ≥ 0.3 after algorithm implementation.".to_string(),
                });
            }

            _ => {}
        }
    }

    diags
}
