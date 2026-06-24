use crate::observations::Observation;
use std::collections::HashSet;
use std::path::Path;

/// Parse a `.tera` template file and detect TPL-001: template consumes a variable
/// that the paired SPARQL SELECT does not produce.
///
/// Looks for a sibling `.rq` file with the same stem to extract projected variables.
/// If no paired `.rq` exists, skip (no diagnostic — incomplete information).
pub fn parse_tera_template(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    let path = Path::new(filepath);
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return obs,
    };

    // Convention: paired SPARQL query is <stem>.rq in same directory
    let rq_path = path.with_file_name(format!("{stem}.rq"));
    let sparql_vars: HashSet<String> = if rq_path.is_file() {
        match std::fs::read_to_string(&rq_path) {
            Ok(rq_content) => extract_sparql_select_vars(&rq_content),
            Err(_) => return obs, // unreadable .rq — skip
        }
    } else {
        return obs; // no paired .rq — skip
    };

    // Wildcard SELECT * means we can't validate
    if sparql_vars.is_empty() {
        return obs;
    }

    let tera_builtins: HashSet<&str> = [
        "loop",
        "self",
        "super",
        "config",
        "now",
        "range",
        "throw",
        "sparql_results",
        "row",
        "rows",
        "true",
        "false",
    ]
    .iter()
    .copied()
    .collect();

    for (var_name, line_num) in extract_tera_variables(content) {
        if tera_builtins.contains(var_name.as_str()) {
            continue;
        }
        let var_lower = var_name.to_lowercase();
        if !sparql_vars.contains(&var_lower) {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: 0,
                end_byte: 0,
                line: line_num,
                column: 1,
                kind: "ggen_template".to_string(),
                construct: "ggen_template_var_mismatch".to_string(),
                context: format!(
                    "Template uses '{}' but {} does not project it. Projects: [{}]",
                    var_name,
                    rq_path.display(),
                    sparql_vars.iter().cloned().collect::<Vec<_>>().join(", ")
                ),
                message: format!(
                    "TPL-001: template variable '{var_name}' not in paired SPARQL SELECT"
                ),
            });
        }
    }

    obs
}

/// Extract `{{ varname }}` root variable names from a Tera template.
/// Returns `(variable_name, line_number)` pairs. Only the root name before `.` or `|`.
fn extract_tera_variables(content: &str) -> Vec<(String, usize)> {
    let mut vars = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let mut search = line;
        while let Some(start) = search.find("{{") {
            let rest = &search[start + 2..];
            if let Some(end) = rest.find("}}") {
                let expr = rest[..end].trim();
                // Root variable: before first '.', ' ', or '|'
                let root = expr.split(['.', ' ', '|']).next().unwrap_or("").trim();
                if !root.is_empty()
                    && root.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && !root
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                {
                    vars.push((root.to_string(), line_num));
                }
                search = &rest[end + 2..];
            } else {
                break;
            }
        }
    }
    vars
}

/// Extract projected `?variable` names from a SPARQL SELECT clause.
/// Returns lowercase names. Empty set means SELECT * (wildcard — can't validate).
fn extract_sparql_select_vars(content: &str) -> HashSet<String> {
    let mut vars = HashSet::new();
    let upper = content.to_uppercase();

    let select_pos = match upper.find("SELECT") {
        Some(p) => p,
        None => return vars,
    };

    let after_select = &content[select_pos + 6..];
    let trimmed = after_select.trim_start();

    // SELECT * WHERE — wildcard, return empty to signal "can't validate"
    if trimmed.starts_with('*') {
        return vars;
    }

    let where_pos = after_select
        .to_uppercase()
        .find("WHERE")
        .unwrap_or(after_select.len());
    let projection = &after_select[..where_pos];

    for token in projection.split_whitespace() {
        if let Some(rest) = token.strip_prefix('?') {
            vars.insert(rest.to_lowercase());
        }
    }

    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tera_vars() {
        let tmpl = "Hello {{ breed_id }}, your label is {{ breed_label | upper }}";
        let vars = extract_tera_variables(tmpl);
        let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"breed_id"));
        assert!(names.contains(&"breed_label"));
    }

    #[test]
    fn extracts_sparql_vars() {
        let rq = "SELECT ?breed_id ?breed_label WHERE { ?b compat:breedId ?breed_id . }";
        let vars = extract_sparql_select_vars(rq);
        assert!(vars.contains("breed_id"));
        assert!(vars.contains("breed_label"));
    }

    #[test]
    fn sparql_wildcard_returns_empty() {
        let rq = "SELECT * WHERE { ?s ?p ?o }";
        let vars = extract_sparql_select_vars(rq);
        assert!(vars.is_empty(), "wildcard SELECT should return empty set");
    }
}
