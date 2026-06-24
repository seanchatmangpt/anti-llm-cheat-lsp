use crate::diagnostics::AntiLlmDiagnostic;
use crate::ocel::detections_to_ocel;

/// Virtual document content for `anti-llm://ocel-log`.
/// Returns OCEL 2.0 JSON derived from live detections.
pub fn render(diagnostics: &[AntiLlmDiagnostic]) -> String {
    let log = detections_to_ocel(diagnostics);
    serde_json::to_string_pretty(&log).unwrap_or_else(|_| "{}".to_string())
}
