use crate::observations::Observation;

pub fn parse_cargo_lock(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    // We scan for lockfile entries: name = "tower-lsp"
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.replace(" ", "").contains("name=\"tower-lsp\"") {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: 0,
                end_byte: 0,
                line: line_idx + 1,
                column: 1,
                kind: "cargo_lock".to_string(),
                construct: "tower-lsp lock entry".to_string(),
                context: trimmed.to_string(),
                message: "Plain tower-lsp dependency found in Cargo.lock".to_string(),
            });
        }
    }

    obs
}
