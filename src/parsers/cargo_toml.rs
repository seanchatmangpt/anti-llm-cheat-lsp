use crate::observations::Observation;

/// CalVer pattern: YY.M.D  (e.g. 26.6.12)
fn is_calver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        && parts[0].len() == 2
        && parts[1].len() <= 2
        && parts[2].len() <= 2
}

/// Extract the value from a `key = "value"` line (returns bare value without quotes).
fn extract_quoted_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{} = \"", key);
    let start = line.find(needle.as_str())? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(&line[start..end])
}

pub fn parse_cargo_toml(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();
    let mut in_workspace_package = false;

    // Simple line-based scanning for Cargo dependencies and versions
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Track [workspace.package] section
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
        }

        // Check for tower-lsp dependency
        if (trimmed.contains("tower-lsp") || trimmed.contains("tower_lsp"))
            && !(trimmed.contains("lsp-max") || trimmed.contains("lsp_max"))
        {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: 0,
                end_byte: 0,
                line: line_idx + 1,
                column: 1,
                kind: "cargo_toml".to_string(),
                construct: "tower-lsp dependency".to_string(),
                context: trimmed.to_string(),
                message: "Plain tower-lsp found in Cargo dependency declaration".to_string(),
            });
        }

        // Check for version = "1.0.0" or v1.0.0
        if trimmed.replace(" ", "").contains("version=\"1.0.0\"") {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: 0,
                end_byte: 0,
                line: line_idx + 1,
                column: 1,
                kind: "cargo_toml".to_string(),
                construct: "version = \"1.0.0\"".to_string(),
                context: trimmed.to_string(),
                message: "Default template version '1.0.0' found".to_string(),
            });
        }

        // VERSION-002: path dep with explicit non-CalVer version
        // Matches inline table lines like: my-lib = { path = "...", version = "1.2.3" }
        if trimmed.contains("path =") {
            if let Some(ver) = extract_quoted_value(trimmed, "version") {
                if !is_calver(ver) {
                    obs.push(Observation {
                        file_path: filepath.to_string(),
                        start_byte: 0,
                        end_byte: 0,
                        line: line_idx + 1,
                        column: 1,
                        kind: "cargo_toml".to_string(),
                        construct: "path_dep_with_semver_version".to_string(),
                        context: trimmed.to_string(),
                        message: format!(
                            "Path dependency declares explicit non-CalVer version '{ver}'"
                        ),
                    });
                }
            }
        }

        // VERSION-003: [workspace.package] with non-CalVer version
        if in_workspace_package {
            if let Some(ver) = extract_quoted_value(trimmed, "version") {
                if !is_calver(ver) {
                    obs.push(Observation {
                        file_path: filepath.to_string(),
                        start_byte: 0,
                        end_byte: 0,
                        line: line_idx + 1,
                        column: 1,
                        kind: "cargo_toml".to_string(),
                        construct: "workspace_semver_version".to_string(),
                        context: trimmed.to_string(),
                        message: format!("Workspace package declares non-CalVer version '{ver}'"),
                    });
                }
            }
        }
    }

    obs
}
