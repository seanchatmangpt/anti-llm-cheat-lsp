use crate::{diagnostics::AntiLlmDiagnostic, observations::Observation};

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        if o.kind != "contract_schism" {
            continue;
        }

        match o.construct.as_str() {
            // CONTRACT-001: zero vocabulary overlap between impl and oracle test
            "contract_vocab_divergence" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-CONTRACT-001".to_string(),
                    category: "contract".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "TestVocab => ImplVocab".to_string(),
                    blocking: true,
                    required_correction: "Implementation and oracle tests must share function vocabulary. Divergent vocabularies indicate the tests were rewritten to match the implementation (A9 contract schism) rather than validating an independent spec.".to_string(),
                    required_next_proof: "Oracle tests predate or are independently authored from the implementation; CI diff gate confirms no commit modifies both impl and oracle simultaneously.".to_string(),
                });
            }

            // CONTRACT-002: function shadow override
            "contract_fn_shadow" => {
                diags.push(AntiLlmDiagnostic {
                    code: "ANTI-LLM-CONTRACT-002".to_string(),
                    category: "contract".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "TestFnDef => ImplFnDef".to_string(),
                    blocking: true,
                    required_correction: "Non-trivial function names must not appear in both implementation and test source for the same module. Shadow definitions indicate a test rewriting the production API to match its own vocabulary.".to_string(),
                    required_next_proof: "Remove duplicate definition; oracle test calls through the standard public API without redefining it.".to_string(),
                });
            }

            _ => {}
        }
    }

    diags
}
