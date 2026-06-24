use serde_json::Value;
use std::fs;

pub fn generate_ledger_markdown(receipts_dir: &str) -> String {
    let mut out = String::new();
    out.push_str("# Receipt Ledger\n\n");
    out.push_str("| Receipt Path | Digest Algorithm | Digest | Boundary | Checkpoint | Raw Command | Status |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");

    if let Ok(entries) = fs::read_dir(receipts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<Value>(&content) {
                        let digest = val
                            .get("digest")
                            .and_then(|d| d.as_str())
                            .unwrap_or("unknown");
                        let alg = val
                            .get("digest_algorithm")
                            .and_then(|d| d.as_str())
                            .unwrap_or("unknown");
                        let boundary = val
                            .get("boundary")
                            .and_then(|d| d.as_str())
                            .unwrap_or("unknown");
                        let checkpoint = val
                            .get("checkpoint")
                            .and_then(|d| d.as_str())
                            .unwrap_or("unknown");
                        let cmd = val
                            .get("raw_command")
                            .and_then(|d| d.as_str())
                            .unwrap_or("unknown");
                        let status = val
                            .get("status")
                            .and_then(|d| d.as_str())
                            .unwrap_or("ADMITTED");

                        out.push_str(&format!(
                            "| {} | {} | {} | {} | {} | {} | {} |\n",
                            path.file_name().unwrap().to_string_lossy(),
                            alg,
                            digest,
                            boundary,
                            checkpoint,
                            cmd,
                            status
                        ));
                    }
                }
            }
        }
    }
    out
}
