use lsp_max::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiLlmDiagnostic {
    pub code: String,
    pub category: String,
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub forbidden_implication: String,
    pub blocking: bool,
    pub required_correction: String,
    pub required_next_proof: String,
}

impl AntiLlmDiagnostic {
    pub fn to_lsp(&self) -> Diagnostic {
        let start_pos = Position::new(
            (self.line.saturating_sub(1)) as u32,
            (self.column.saturating_sub(1)) as u32,
        );
        let end_pos = Position::new(
            (self.line.saturating_sub(1)) as u32,
            (self.column.saturating_sub(1) + 10) as u32,
        );

        let severity = if self.blocking {
            DiagnosticSeverity::ERROR
        } else {
            DiagnosticSeverity::WARNING
        };

        Diagnostic {
            range: Range::new(start_pos, end_pos),
            severity: Some(severity),
            code: Some(lsp_max::lsp_types::NumberOrString::String(
                self.code.clone(),
            )),
            source: Some("anti-llm-cheat-lsp".to_string()),
            message: format!(
                "{}\nForbidden Implication: {}\nRequired Correction: {}\nRequired Next Proof: {}",
                self.message,
                self.forbidden_implication,
                self.required_correction,
                self.required_next_proof
            ),
            ..Default::default()
        }
    }
}
