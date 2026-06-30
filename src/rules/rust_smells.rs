use crate::{diagnostics::AntiLlmDiagnostic, observations::Observation};

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // Debug diagnostic names found
        if o.construct == "CLAP-DEBUG" || o.construct == "CLAP-DEBUG-PATH" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-001".to_string(),
                category: "strange-code".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Debug diagnostic name found in admissible path.".to_string(),
                forbidden_implication: "Debug scaffold => law diagnostic".to_string(),
                blocking: true,
                required_correction:
                    "Remove temporary/debug diagnostics from production code paths.".to_string(),
                required_next_proof: "Verify all diagnostics are production-ready.".to_string(),
            });
        }

        // Diagnostic leaks raw content
        if o.construct == "Content was:" || o.message.contains("leaks raw content") {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-002".to_string(),
                category: "strange-code".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message:
                    "Diagnostic leaks raw file content, which could leak secrets or private data."
                        .to_string(),
                forbidden_implication: "Raw content dump => useful diagnostic".to_string(),
                blocking: true,
                required_correction:
                    "Obfuscate or summarize content in diagnostics instead of printing raw content."
                        .to_string(),
                required_next_proof: "Check diagnostic message serialization.".to_string(),
            });
        }

        // Diagnostic leaks raw path
        if o.construct == "Path was:" || o.message.contains("leaks raw path") {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-003".to_string(),
                category: "strange-code".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Diagnostic leaks raw path, violating environment isolation rules."
                    .to_string(),
                forbidden_implication: "Raw path dump => law diagnostic".to_string(),
                blocking: true,
                required_correction: "Output relative or sanitized paths in diagnostic details."
                    .to_string(),
                required_next_proof: "Check path scrubbing function in diagnostic emitter."
                    .to_string(),
            });
        }

        // Substring check used as law
        if o.construct.starts_with("content.contains")
            || o.construct.starts_with("path.ends_with")
            || o.construct.starts_with("path_str.contains")
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-007".to_string(),
                category: "strange-code".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Substring check used as law (e.g. searching 'customization-map.json' or 'TODO').".to_string(),
                forbidden_implication: "SubstringMatch => Authority".to_string(),
                blocking: true,
                required_correction: "Use structural AST or file metadata parsing instead of simple string searches for policy checks.".to_string(),
                required_next_proof: "Verify utilizing tree-sitter or JSON-TOML deserializers.".to_string(),
            });
        }

        // STRANGE-008: println! left in production code
        if o.construct == "println_in_production" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-008".to_string(),
                category: "strange-code".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "println! found in non-test production code — debug output left in"
                    .to_string(),
                forbidden_implication: "DebugPrint => ProductionCode".to_string(),
                blocking: true,
                required_correction:
                    "Remove println! or replace with tracing::debug!/info! for structured logging."
                        .to_string(),
                required_next_proof: "Verify no unintended debug output in production paths."
                    .to_string(),
            });
        }

        // STRANGE-009: #[cfg(test)] in a non-test file — law evasion by hiding code in test cfg
        if o.construct == "cfg_test_in_production" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-009".to_string(),
                category: "strange-code".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message:
                    "#[cfg(test)] in a non-test file — production logic must not be gated by test cfg"
                        .to_string(),
                forbidden_implication: "CfgTest => ProductionLogicEvasion".to_string(),
                blocking: true,
                required_correction:
                    "Move test-only code to a dedicated test file or tests/ directory."
                        .to_string(),
                required_next_proof:
                    "Verify that all #[cfg(test)] blocks are in test files only."
                        .to_string(),
            });
        }
    }

    diags
}
