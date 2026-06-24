use crate::observations::Observation;
use serde_json::Value;

pub fn parse_receipt_json(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    if let Ok(val) = serde_json::from_str::<Value>(content) {
        // Enforce required fields
        let required_fields = [
            "digest",
            "digest_algorithm",
            "boundary",
            "checkpoint",
            "raw_command",
            "output_digest",
        ];
        for field in &required_fields {
            if val.get(field).is_none() {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: 1,
                    column: 1,
                    kind: "receipt_json".to_string(),
                    construct: format!("missing {}", field),
                    context: content.to_string(),
                    message: format!("Receipt file lacks required field '{}'", field),
                });
            }
        }

        // Enforce BLAKE3 for Gall receipts
        if let Some(alg) = val.get("digest_algorithm").and_then(|a| a.as_str()) {
            if alg != "BLAKE3" && alg != "SHA-256" {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: 1,
                    column: 1,
                    kind: "receipt_json".to_string(),
                    construct: "invalid digest_algorithm".to_string(),
                    context: content.to_string(),
                    message: format!(
                        "Receipt uses invalid digest algorithm '{}'; expected BLAKE3 or SHA-256",
                        alg
                    ),
                });
            }
        }
    } else {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: 0,
            end_byte: 0,
            line: 1,
            column: 1,
            kind: "receipt_json".to_string(),
            construct: "invalid json".to_string(),
            context: content.to_string(),
            message: "Receipt file is not valid JSON".to_string(),
        });
    }

    obs
}
