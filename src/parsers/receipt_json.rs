use serde_json::Value;

use crate::observations::Observation;

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

        // RECEIPT-004: obviously fabricated digest value
        if let Some(digest) = val.get("digest").and_then(|d| d.as_str()) {
            let is_all_zeros = digest == "0".repeat(64).as_str()
                || digest.chars().all(|c| c == '0') && digest.len() >= 32;
            // Repeated single character (aaaa...  or 1111...)
            let first = digest.chars().next().unwrap_or('_');
            let is_repeated_char = digest.len() >= 16 && digest.chars().all(|c| c == first);
            // Well-known test/placeholder hex strings
            let dl = digest.to_lowercase();
            let is_known_placeholder = dl.starts_with("deadbeef")
                || dl.starts_with("cafebabe")
                || dl.starts_with("12345678")
                || dl.starts_with("abcdefab");
            if is_all_zeros || is_repeated_char || is_known_placeholder {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: 1,
                    column: 1,
                    kind: "receipt_json".to_string(),
                    construct: "fabricated_digest".to_string(),
                    context: format!("digest={}", &digest[..digest.len().min(24)]),
                    message: "Receipt digest is obviously fabricated (all-zeros, repeated char, or well-known placeholder)".to_string(),
                });
            }
        }

        // RECEIPT-005: self-referential receipt — "path" field names the receipt file itself
        if let Some(path_val) = val.get("path").and_then(|p| p.as_str()) {
            let receipt_stem = std::path::Path::new(filepath)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            // Strip .receipt suffix if present to get the base name
            let base = receipt_stem.strip_suffix(".receipt").unwrap_or(receipt_stem);
            let filename_only = std::path::Path::new(filepath)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("");
            if (!base.is_empty() && path_val.contains(base))
                || path_val.contains(filename_only)
            {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: 1,
                    column: 1,
                    kind: "receipt_json".to_string(),
                    construct: "self_referential_receipt".to_string(),
                    context: format!("path={}", path_val),
                    message: "Receipt 'path' field references the receipt file itself — self-referential receipt".to_string(),
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
