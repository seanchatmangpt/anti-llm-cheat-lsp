use crate::observations::Observation;
use serde_json::Value;

pub fn parse_json_rpc_transcript(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    // Transcripts are usually JSONL format
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            // Check for initialize request and verify client capabilities
            if val.get("method").and_then(|m| m.as_str()) == Some("initialize") {
                if let Some(params) = val.get("params") {
                    let mut has_3_18_capabilities = false;

                    // Check if client capabilities advertise 3.18 inlineCompletion or textDocumentContent
                    if let Some(text_doc) = params
                        .get("capabilities")
                        .and_then(|c| c.get("textDocument"))
                    {
                        if text_doc.get("inlineCompletion").is_some()
                            || text_doc.get("foldingRange").is_some()
                        {
                            has_3_18_capabilities = true;
                        }
                    }

                    if !has_3_18_capabilities {
                        obs.push(Observation {
                            file_path: filepath.to_string(),
                            start_byte: 0,
                            end_byte: 0,
                            line: line_idx + 1,
                            column: 1,
                            kind: "json_rpc".to_string(),
                            construct: "initialize without 3.18 caps".to_string(),
                            context: trimmed.to_string(),
                            message: "LSP 3.18 initialize transcript lacks advertised client capabilities".to_string(),
                        });
                    }
                }
            }
        }
    }

    obs
}
