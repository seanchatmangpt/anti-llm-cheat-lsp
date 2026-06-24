use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

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
    }

    // Check for warnings emitted for non-error states
    // In our diagnostic rules, warning states should not be emitted for non-error states
    // but this is mostly handled by custom checks on the diagnostic severity.

    diags
}
