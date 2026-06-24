use crate::observations::Observation;

/// Parse a single `ggen.toml` file and emit observations for GGEN-* rule evaluation.
///
/// Produces:
/// - `ggen_second_class_path` — `output_file` contains `/generated/`, `/output/`, `/gen/`
/// - `ggen_layer_violation` — `output_file` has no path separator (targets pack root)
/// - `ggen_remote_fetch` — ontology/imports value starts with `http://` or `https://`
/// - `ggen_output_file_decl` — all output_file values (used by YIELD-004 cross-file dedup)
pub fn parse_ggen_toml(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    const SECOND_CLASS_SEGMENTS: &[&str] = &[
        "/generated/",
        "/output/",
        "/gen/",
        "\\generated\\",
        "\\output\\",
        "\\gen\\",
    ];
    const SECOND_CLASS_SUFFIXES: &[&str] = &[
        "/generated",
        "/output",
        "/gen",
        "\\generated",
        "\\output",
        "\\gen",
    ];

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        // Skip comments
        if trimmed.starts_with('#') {
            continue;
        }

        // output_file = "..." detection
        if let Some(value) = extract_toml_string_value(trimmed, "output_file") {
            // Emit a general output_file declaration for cross-file YIELD-004 dedup
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: 0,
                end_byte: 0,
                line: line_num,
                column: 1,
                kind: "ggen_output_file".to_string(),
                construct: value.to_string(),
                context: line.to_string(),
                message: format!("ggen.toml declares output_file = \"{value}\""),
            });

            // SRC-001 / YIELD-002: second-class path segments
            let has_second_class = SECOND_CLASS_SEGMENTS.iter().any(|seg| value.contains(seg))
                || SECOND_CLASS_SUFFIXES.iter().any(|suf| value.ends_with(suf));
            if has_second_class {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "ggen_manifest".to_string(),
                    construct: "ggen_second_class_path".to_string(),
                    context: value.to_string(),
                    message: format!(
                        "output_file contains second-class segment (generated/output/gen): {value}"
                    ),
                });
            }

            // YIELD-001: output_file targets pack root — no directory component
            if !value.contains('/') && !value.contains('\\') && !value.is_empty() {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "ggen_manifest".to_string(),
                    construct: "ggen_layer_violation".to_string(),
                    context: value.to_string(),
                    message: format!(
                        "output_file '{value}' has no directory component — targets pack root"
                    ),
                });
            }
        }

        // YIELD-005: remote ontology fetch detection
        // Matches `ontology = "https://..."` and array elements `"https://..."` in imports
        if trimmed.starts_with("source")
            || trimmed.starts_with("ontology")
            || trimmed.starts_with("imports")
            || trimmed.starts_with('"')
        {
            for key in &["source", "ontology"] {
                if let Some(value) = extract_toml_string_value(trimmed, key) {
                    if value.starts_with("http://") || value.starts_with("https://") {
                        obs.push(Observation {
                            file_path: filepath.to_string(),
                            start_byte: 0,
                            end_byte: 0,
                            line: line_num,
                            column: 1,
                            kind: "ggen_manifest".to_string(),
                            construct: "ggen_remote_fetch".to_string(),
                            context: value.to_string(),
                            message: format!("Remote ontology fetch in replay path: {value}"),
                        });
                    }
                }
            }
            // Also catch bare string array elements: `  "https://..."`
            if trimmed.starts_with('"') {
                let value = trimmed.trim_matches('"').trim_matches(',');
                if value.starts_with("http://") || value.starts_with("https://") {
                    obs.push(Observation {
                        file_path: filepath.to_string(),
                        start_byte: 0,
                        end_byte: 0,
                        line: line_num,
                        column: 1,
                        kind: "ggen_manifest".to_string(),
                        construct: "ggen_remote_fetch".to_string(),
                        context: value.to_string(),
                        message: format!("Remote ontology fetch in replay path: {value}"),
                    });
                }
            }
        }
    }

    obs
}

/// Extract the string value from a TOML key-value line like `key = "value"`.
/// Returns the inner string without quotes, or None if the line doesn't match.
fn extract_toml_string_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    // Match `key = "..."` or `key="..."`
    let stripped = if let Some(rest) = line.strip_prefix(key) {
        rest.trim_start()
    } else {
        return None;
    };
    let stripped = stripped.strip_prefix('=')?;
    let stripped = stripped.trim();
    if stripped.starts_with('"') && stripped.contains('"') {
        let inner = &stripped[1..];
        let end = inner.find('"')?;
        Some(&inner[..end])
    } else {
        None
    }
}

/// Detect GGEN-YIELD-004: two ggen.toml manifests declare the same output_file.
/// Takes all observations from all ggen.toml files in a workspace scan.
pub fn detect_competing_authority(all_obs: &[Observation]) -> Vec<Observation> {
    use std::collections::HashMap;
    // output_file_value -> Vec<(manifest_path, line)>
    let mut seen: HashMap<String, Vec<(String, usize)>> = HashMap::new();

    for o in all_obs {
        if o.kind == "ggen_output_file" {
            seen.entry(o.construct.clone())
                .or_default()
                .push((o.file_path.clone(), o.line));
        }
    }

    let mut extra = Vec::new();
    for (output_path, locations) in &seen {
        if locations.len() < 2 {
            continue;
        }
        for (i, (file_path, line)) in locations.iter().enumerate() {
            let others: Vec<&str> = locations
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, (p, _))| p.as_str())
                .collect();
            extra.push(Observation {
                file_path: file_path.clone(),
                start_byte: 0,
                end_byte: 0,
                line: *line,
                column: 1,
                kind: "ggen_manifest".to_string(),
                construct: "ggen_competing_authority".to_string(),
                context: format!("{output_path} (also claimed by: {})", others.join(", ")),
                message: format!(
                    "COMPETING_AUTHORITY: output_file \"{output_path}\" also declared in: {}",
                    others.join(", ")
                ),
            });
        }
    }
    extra
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_second_class_path() {
        let content = r#"
[[generation.rules]]
output_file = "src/generated/breeds.rs"
"#;
        let obs = parse_ggen_toml("ggen.toml", content);
        assert!(obs.iter().any(|o| o.construct == "ggen_second_class_path"));
    }

    #[test]
    fn detects_remote_fetch() {
        let content = r#"
[ontology]
source = "https://example.com/ontology.ttl"
"#;
        let obs = parse_ggen_toml("ggen.toml", content);
        assert!(obs.iter().any(|o| o.construct == "ggen_remote_fetch"));
    }

    #[test]
    fn clean_ggen_toml_no_violations() {
        let content = r#"
[[generation.rules]]
output_file = "src/fresh_names.rs"
"#;
        let obs = parse_ggen_toml("ggen.toml", content);
        // Should have one output_file_decl but no violations
        assert!(obs.iter().all(|o| o.kind == "ggen_output_file"));
    }

    #[test]
    fn detects_competing_authority() {
        let a = parse_ggen_toml("/project/a/ggen.toml", "output_file = \"src/shared.rs\"\n");
        let b = parse_ggen_toml("/project/b/ggen.toml", "output_file = \"src/shared.rs\"\n");
        let all: Vec<_> = a.into_iter().chain(b).collect();
        let conflicts = detect_competing_authority(&all);
        assert_eq!(conflicts.len(), 2, "both manifests should be flagged");
        assert!(conflicts
            .iter()
            .all(|o| o.construct == "ggen_competing_authority"));
    }
}
