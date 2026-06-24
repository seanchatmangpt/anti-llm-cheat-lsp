use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        if o.kind == "ts_smell" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-STRANGE-009".to_string(),
                category: "typescript-smell".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!("TypeScript code smell or stub detected: {}.", o.message),
                forbidden_implication: "TypeScriptStub => AdmissibleCode".to_string(),
                blocking: true,
                required_correction:
                    "Remove ts-ignore, eslint-disable, unsafe casting (as any), or TODO stubs."
                        .to_string(),
                required_next_proof: "Ensure typescript compiles strictly and uses explicit types."
                    .to_string(),
            });
        }

        if o.kind == "ts_claim" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-CLAIM-005".to_string(),
                category: "typescript-claim".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!("Forbidden victory/done claim: {}.", o.message),
                forbidden_implication: "StatusWord(CLAIMED) => Admitted".to_string(),
                blocking: true,
                required_correction: "Remove forbidden done/complete/victory words and replace with bounded status vocabulary.".to_string(),
                required_next_proof: "Admissibility audit checks TS surfaces.".to_string(),
            });
        }

        if o.kind == "ts_leak" {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-CLAIM-006".to_string(),
                category: "typescript-leak".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!("Vocabulary or naming fence violation in TypeScript: {}.", o.message),
                forbidden_implication: "InternalTermLeak => PublicSurface".to_string(),
                blocking: true,
                required_correction: "Sanitize internal vocabulary terms (GALL, checkpoint, failset, etc.) or unauthorized naming (Nitro LSP) from TypeScript files.".to_string(),
                required_next_proof: "Verify all file paths and contents comply with the public-facing language boundary.".to_string(),
            });
        }
    }

    diags
}
