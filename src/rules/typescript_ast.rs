// rules/typescript_ast.rs — diagnostic rules for AST-level TypeScript cheat detections.
//
// Maps observations from parsers::typescript_ast to AntiLlmDiagnostic structs.
// Each rule maps one observation kind to one diagnostic code.

use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        match o.kind.as_str() {
            // STRANGE-012: SHA-256 algorithm literal
            "ast_ts_sha256" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-STRANGE-012".to_string(),
                    category: "typescript-sha256".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("[TS-AST] {}", o.message),
                    forbidden_implication: "SHA256Call => Blake3Required".to_string(),
                    blocking: true,
                    required_correction:
                        "Replace SHA-256 algorithm string with BLAKE3 via @noble/hashes/blake3.js"
                            .to_string(),
                    required_next_proof:
                        "Verify all hash calls use blake3() from @noble/hashes/blake3.js"
                            .to_string(),
                });
            }

            // STRANGE-013: Non-deterministic call (Math.random / Date.now)
            "ast_ts_nondeterminism" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-STRANGE-013".to_string(),
                    category: "typescript-nondeterminism".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("[TS-AST] {}", o.message),
                    forbidden_implication: "NonDeterministicCall => ReplayLawViolation".to_string(),
                    blocking: false, // warning — some uses are legitimate
                    required_correction:
                        "Pass timestamps/random seeds via args or use seeded RNG for replay-safe code".to_string(),
                    required_next_proof:
                        "Audit all Date.now() / Math.random() calls for replay compliance".to_string(),
                });
            }

            // STRANGE-014: Test double (mock/spy) outside test file
            "ast_ts_mock_leak" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-STRANGE-014".to_string(),
                    category: "typescript-mock-leak".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("[TS-AST] {}", o.message),
                    forbidden_implication: "TestDouble(Production) => ObservabilityBreached".to_string(),
                    blocking: true,
                    required_correction:
                        "Move vi.mock/jest.mock to test files only; production code must not mock its own dependencies".to_string(),
                    required_next_proof:
                        "Confirm no mock() calls remain in non-test TypeScript sources".to_string(),
                });
            }

            // STRANGE-016: Hardcoded 64-char hex oracle hash in test assertion
            "ast_ts_oracle_hash" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-STRANGE-016".to_string(),
                    category: "typescript-oracle-hash".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("[TS-AST] {}", o.message),
                    forbidden_implication: "HardcodedHash => OracleBreach".to_string(),
                    blocking: true,
                    required_correction:
                        "Compute expected hash dynamically: blake3Hex(input) not a literal string".to_string(),
                    required_next_proof:
                        "All 64-hex assertions must derive from the same algorithm call as production".to_string(),
                });
            }

            // STRANGE-017: crypto.subtle.digest('SHA-256') call
            "ast_ts_sha256_digest" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-STRANGE-017".to_string(),
                    category: "typescript-webcrypto-sha256".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("[TS-AST] {}", o.message),
                    forbidden_implication: "WebCryptoSHA256 => Blake3Required".to_string(),
                    blocking: true,
                    required_correction:
                        "Replace crypto.subtle.digest('SHA-256', ...) with blake3() from @noble/hashes/blake3.js — WebCrypto does not support BLAKE3".to_string(),
                    required_next_proof:
                        "Audit all crypto.subtle.digest calls; none should reference SHA-256".to_string(),
                });
            }

            // STRANGE-018: console.log in server route
            "ast_ts_console_leak" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-STRANGE-018".to_string(),
                    category: "typescript-console-leak".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("[TS-AST] {}", o.message),
                    forbidden_implication: "ConsoleLeak(ServerRoute) => PIIExposureRisk".to_string(),
                    blocking: false, // warning
                    required_correction:
                        "Replace console.log/error/warn in server routes with structured logging (e.g., useLogger or silent drop)".to_string(),
                    required_next_proof:
                        "Server routes must not log raw request/response bodies to stdout".to_string(),
                });
            }

            _ => {}
        }
    }

    diags
}
