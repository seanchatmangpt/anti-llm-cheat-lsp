use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

fn is_breed_src(path: &str) -> bool {
    path.contains("breeds/") || path.contains("src/breeds")
}

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
            // ORACLE-001: lazy_static initialized from env
            "lazy_static_env_init" if !is_test_path(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-ORACLE-001".to_string(),
                    category: "oracle".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "lazy_static! or OnceLock initialized from env var in '{}' — oracle injection via environment.",
                        o.context
                    ),
                    forbidden_implication: "EnvInit => DeterministicAlgorithm".to_string(),
                    blocking: true,
                    required_correction: "Remove env-var-driven static initialization from production breed code. All algorithm inputs must flow through the BreedInput contract.".to_string(),
                    required_next_proof: "Verify breed produces correct output with env vars unset. Oracle hidden test with novel inputs must pass.".to_string(),
                });
            }

            // ORACLE-002: transmute / raw pointer cast
            "transmute_cast" if !is_test_path(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-ORACLE-002".to_string(),
                    category: "oracle".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "mem::transmute or raw pointer cast in '{}' — type erasure cheat path.",
                        o.context
                    ),
                    forbidden_implication: "TypeErasure => SafeComputation".to_string(),
                    blocking: true,
                    required_correction: "Remove transmute/pointer casts. Use typed transformations. Type erasure can hide oracle injection from static analysis.".to_string(),
                    required_next_proof: "Zero unsafe pointer operations in breed implementation; cargo test passes without transmute.".to_string(),
                });
            }

            // ORACLE-003: global HashMap literal (string key → value memo table)
            "global_hashmap_literal" if is_breed_src(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-ORACLE-003".to_string(),
                    category: "oracle".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "HashMap built from string literal keys in '{}' — global memo oracle table.",
                        o.context
                    ),
                    forbidden_implication: "GlobalMemoTable => AlgorithmExecution".to_string(),
                    blocking: true,
                    required_correction: "Remove HashMap literal lookup table from breed source. Algorithm must compute results, not retrieve them from an embedded key-value map.".to_string(),
                    required_next_proof: "Hidden oracle with keys not present in the removed map must still produce correct output via computation.".to_string(),
                });
            }

            // ORACLE-004: std::env::var in production path
            "env_var_in_prod" if !is_test_path(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-ORACLE-004".to_string(),
                    category: "oracle".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "std::env::var() called in production code path '{}' — environment oracle channel.",
                        o.context
                    ),
                    forbidden_implication: "EnvRead => AlgorithmInput".to_string(),
                    blocking: true,
                    required_correction: "Move env var access outside breed implementation. Configuration must flow through BreedInput.options, not environment side-channels.".to_string(),
                    required_next_proof: "Breed runs correctly with a clean environment (no vars set). Novel input oracle must pass.".to_string(),
                });
            }

            // ORACLE-005: single-expression trait impl (suspicious but not blocking alone)
            "trait_impl_single_expr" if is_breed_src(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-ORACLE-005".to_string(),
                    category: "oracle".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "Trait impl method '{}' has minimal body (≤2 statements) in breed source — possible memorization impl.",
                        o.context
                    ),
                    forbidden_implication: "SingleExprImpl => AlgorithmDispatch".to_string(),
                    blocking: false,
                    required_correction: "If this is a real trait delegation, document why. If it wraps oracle logic, implement the full algorithm.".to_string(),
                    required_next_proof: "Method body invokes multiple sub-operations; oracle test with novel inputs passes.".to_string(),
                });
            }

            // ORACLE-006: float literal in known oracle value range in breed src
            "const_suspicious_float" if is_breed_src(&o.file_path) => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-ORACLE-006".to_string(),
                    category: "oracle".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!(
                        "Float literal in breed source matches known paper oracle value range — forensic evidence of oracle answer injection. Context: {}",
                        o.context.chars().take(80).collect::<String>()
                    ),
                    forbidden_implication: "OracleFloat => ComputedOutput".to_string(),
                    blocking: true,
                    required_correction: "Remove float literal matching known oracle answer (Pearl 0.284, MYCIN 0.693, POMDP 0.969, etc.). The algorithm must compute this value, not embed it.".to_string(),
                    required_next_proof: "After removal, hidden oracle test with non-round CPTs still converges to the correct value via computation.".to_string(),
                });
            }

            _ => {}
        }
    }

    diags
}
