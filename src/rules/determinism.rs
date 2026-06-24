use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

fn is_test_path(path: &str) -> bool {
    // Negative-control fixtures must trigger diagnostics — do not exclude them.
    if path.contains("negative_controls/") || path.contains("negative_controls\\") {
        return false;
    }
    path.contains("tests/")
        || path.ends_with("_test.rs")
        || path.contains("/test/")
        || path.contains("fixtures/")
}

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // CHEAT-001: hardcoded metrics (let fitness =, let score =, etc.)
        if o.kind == "raw_text"
            && matches!(
                o.construct.as_str(),
                "let fitness ="
                    | "let score ="
                    | "let precision ="
                    | "let recall ="
                    | "let f1_score ="
            )
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-CHEAT-001".to_string(),
                category: "determinism".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!(
                    "Hardcoded metric variable '{}' detected — oracle law violation. Metrics must be computed, not assigned.",
                    o.construct
                ),
                forbidden_implication: "HardcodedMetric => ComputedResult".to_string(),
                blocking: true,
                required_correction: "Remove hardcoded metric and compute from algorithm output.".to_string(),
                required_next_proof: "Run conformance check and capture live metric output.".to_string(),
            });
        }

        // CHEAT-002: seeded RNG (not in test paths)
        let is_seeded_rng = (o.kind == "ast_node"
            && matches!(
                o.construct.as_str(),
                "seed_from_u64"
                    | "SmallRng::from_seed"
                    | "StdRng::from_seed"
                    | "SeedableRng::seed_from_u64"
                    | "ChaCha8Rng::from_seed"
                    | "from_seed"
            ))
            || (o.kind == "raw_text"
                && matches!(
                    o.construct.as_str(),
                    "seed_from_u64" | "SmallRng::from_seed" | "StdRng::from_seed"
                ));
        if is_seeded_rng && !is_test_path(&o.file_path) {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-CHEAT-002".to_string(),
                category: "determinism".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Seeded RNG detected in non-test code — oracle law violation. Seeded RNG produces predictable output that can be gamed.".to_string(),
                forbidden_implication: "SeededRNG => DeterministicOutput".to_string(),
                blocking: true,
                required_correction: "Remove seeded RNG from production code. Use crypto-secure random or algorithmic output.".to_string(),
                required_next_proof: "Verify no seeded RNG in production paths.".to_string(),
            });
        }

        // CHEAT-003: copied output hash
        if o.kind == "raw_text" && o.construct == "\"output_hash\": \"" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-CHEAT-003".to_string(),
                category: "determinism".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Hardcoded output_hash literal detected — receipt fabrication risk."
                    .to_string(),
                forbidden_implication: "HardcodedHash => ValidReceipt".to_string(),
                blocking: true,
                required_correction: "Compute output_hash from actual algorithm output at runtime."
                    .to_string(),
                required_next_proof: "Verify output_hash is computed, not copied.".to_string(),
            });
        }

        // STRANGE-010: #[allow(...)] suppression cheat (not in test paths)
        if o.kind == "ast_node" && o.construct == "allow_cheat_attr" && !is_test_path(&o.file_path)
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-010".to_string(),
                category: "determinism".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Suppression attribute #[allow(...)] for dead_code/unused/warnings detected — silences real violations.".to_string(),
                forbidden_implication: "AllowSuppression => CleanBuild".to_string(),
                blocking: false,
                required_correction: "Remove suppression attribute and fix the underlying issue.".to_string(),
                required_next_proof: "Build without suppression attribute and verify zero warnings.".to_string(),
            });
        }

        // STRANGE-011: unsafe block or function (not in test paths)
        if o.kind == "ast_node"
            && matches!(o.construct.as_str(), "unsafe_block" | "unsafe_fn_or_impl")
            && !is_test_path(&o.file_path)
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-011".to_string(),
                category: "determinism".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!(
                    "Unsafe {} detected — potential oracle law bypass or memory safety violation.",
                    if o.construct == "unsafe_block" { "block" } else { "function/impl" }
                ),
                forbidden_implication: "UnsafeCode => SafeExecution".to_string(),
                blocking: false,
                required_correction: "Replace unsafe code with safe alternatives or document necessity with a receipt.".to_string(),
                required_next_proof: "Audit unsafe usage and confirm necessity with process evidence.".to_string(),
            });
        }
    }

    diags
}
