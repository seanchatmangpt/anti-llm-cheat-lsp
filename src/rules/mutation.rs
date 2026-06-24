use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // Direct file write in LSP authority path
        if o.construct == "std::fs::write"
            || o.construct == "tokio::fs::write"
            || o.construct == "File::create"
            || o.construct == "OpenOptions"
            || o.construct == "write_all"
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-MUT-001".to_string(),
                category: "mutation".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Direct file write or file creation found in LSP authority path. The server is read-only by default.".to_string(),
                forbidden_implication: "LSP observation => mutation authority".to_string(),
                blocking: true,
                required_correction: "Remove direct file write call. Route mutation requests via CodeAction to PackPlan intent instead.".to_string(),
                required_next_proof: "Verify with read-only permission checks.".to_string(),
            });
        }

        // WorkspaceEdit used as receipt binding
        if o.construct == "WorkspaceEdit"
            || o.message.contains("WorkspaceEdit used as receipt binding")
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-MUT-002".to_string(),
                category: "mutation".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "WorkspaceEdit used directly as receipt binding or mutation proof.".to_string(),
                forbidden_implication: "WorkspaceEdit => admitted receipt mutation".to_string(),
                blocking: true,
                required_correction: "WorkspaceEdit must represent a read-only template intent, not the final mutation receipt.".to_string(),
                required_next_proof: "Enforce MutationGate and sign receipts independently.".to_string(),
            });
        }
    }

    diags
}
