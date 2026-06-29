use crate::{config::AntiLlmConfig, diagnostics::AntiLlmDiagnostic, observations::Observation};

/// Suffixes that mark a function as a "correct alternative" that should replace
/// the primary but may have been left dead (never called).
const ALT_SUFFIXES: &[&str] = &[
    "_v2", "_v3", "_alt", "_correct", "_real", "_proper", "_fixed", "_working", "_new", "_better",
];

/// Scan Rust source for function definitions whose names end with an "alt" suffix
/// and that are never called or referenced elsewhere in the same file.
///
/// Called from `engine::scan_file` for `.rs` files. `dead_alt::evaluate` then
/// emits ANTI-LLM-DEAD-ALT-001 for each observation.
pub fn scan_for_dead_alt(filepath: &str, content: &str) -> Vec<Observation> {
    if !filepath.ends_with(".rs") {
        return Vec::new();
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut obs = Vec::new();

    for (line_idx, line) in lines.iter().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim_start();

        // Match `fn <name>` or `pub fn <name>` (with any visibility/async/unsafe prefix)
        let fn_name = extract_fn_name(trimmed);
        let fn_name = match fn_name {
            Some(n) => n,
            None => continue,
        };

        // Check if the name ends with one of the alt suffixes
        let has_alt_suffix =
            ALT_SUFFIXES.iter().any(|suffix| fn_name == *suffix || fn_name.ends_with(suffix));
        if !has_alt_suffix {
            continue;
        }

        // Count how many lines reference this name outside its own definition line.
        // A "reference" is any line (other than the definition line itself) that
        // contains the function name as a substring. This is intentionally broad:
        // it catches call sites, use-in-closures, method references, etc.
        let call_site_count = lines
            .iter()
            .enumerate()
            .filter(|(idx, l)| *idx != line_idx && l.contains(fn_name))
            .count();

        if call_site_count == 0 {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: 0,
                end_byte: 0,
                line: line_num,
                column: 1,
                kind: "dead_alt_smell".to_string(),
                construct: fn_name.to_string(),
                context: line.to_string(),
                message: format!(
                    "Dead alternative function '{}' defined on line {} but never called",
                    fn_name, line_num
                ),
            });
        }
    }

    obs
}

/// Extract the bare function name from a line that begins with a `fn` declaration.
///
/// Handles common prefixes: `pub`, `pub(crate)`, `pub(super)`, `async`, `unsafe`,
/// `pub async`, `pub unsafe`, `pub(crate) async`, etc.
/// Returns `None` if the line is not a `fn` declaration.
fn extract_fn_name(trimmed: &str) -> Option<&str> {
    // Strip known visibility/modifier prefixes until we reach `fn`.
    let mut s = trimmed;

    // Consume optional `pub(...)` or `pub`
    if s.starts_with("pub(") {
        // skip past the closing paren
        let close = s.find(')')?;
        s = s[close + 1..].trim_start();
    } else if s.starts_with("pub") {
        s = s["pub".len()..].trim_start();
    }

    // Consume optional `async` and/or `unsafe` and/or `extern "..."` in any order
    for _ in 0..4 {
        if s.starts_with("async") && s[5..].starts_with(|c: char| c.is_whitespace()) {
            s = s[5..].trim_start();
        } else if s.starts_with("unsafe") && s[6..].starts_with(|c: char| c.is_whitespace()) {
            s = s[6..].trim_start();
        } else if s.starts_with("extern") {
            // extern "C" fn ... — skip to `fn`
            let fn_pos = s.find("fn ")?;
            s = &s[fn_pos..];
            break;
        } else {
            break;
        }
    }

    // Now expect `fn `
    if !s.starts_with("fn ") {
        return None;
    }
    s = s["fn ".len()..].trim_start();

    // The function name is the identifier up to the first non-identifier character.
    let end = s.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some(&s[..end])
}

pub fn evaluate(obs: &[Observation], config: &AntiLlmConfig) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        if o.kind != "dead_alt_smell" {
            continue;
        }
        if config.is_suppression_allowed(&o.file_path) {
            continue;
        }

        diags.push(AntiLlmDiagnostic {
            code: "ANTI-LLM-DEAD-ALT-001".to_string(),
            category: "dead_alternative".to_string(),
            file_path: o.file_path.clone(),
            line: o.line,
            column: o.column,
            message: format!(
                "Dead alternative function '{}' — defined but never called; broken primary may be in use",
                o.construct
            ),
            forbidden_implication: format!(
                "DeadAlternative({}) => BrokenPrimaryInUse",
                o.construct
            ),
            blocking: true,
            required_correction:
                "Either use the correct alternative function or delete it".to_string(),
            required_next_proof:
                "Show that only one version exists and it is the correct one".to_string(),
        });
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_config() -> crate::config::AntiLlmConfig {
        crate::config::AntiLlmConfig::default()
    }

    #[test]
    fn detects_unused_v2_function() {
        let src = r#"
fn run_with_timeout(ms: u64) -> bool {
    false // broken
}

fn run_with_timeout_v2(ms: u64) -> bool {
    true // correct
}

fn main() {
    run_with_timeout(100);
}
"#;
        let obs = scan_for_dead_alt("example.rs", src);
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].construct, "run_with_timeout_v2");

        let diags = evaluate(&obs, &dummy_config());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "ANTI-LLM-DEAD-ALT-001");
        assert!(diags[0].blocking);
    }

    #[test]
    fn no_false_positive_when_alt_is_called() {
        let src = r#"
fn compute_fixed(x: i32) -> i32 { x * 2 }

fn main() {
    let _ = compute_fixed(3);
}
"#;
        let obs = scan_for_dead_alt("example.rs", src);
        assert!(obs.is_empty());
    }

    #[test]
    fn skips_non_rust_files() {
        let src = "fn foo_v2() {}";
        let obs = scan_for_dead_alt("example.py", src);
        assert!(obs.is_empty());
    }

    #[test]
    fn detects_alt_suffix_variants() {
        for suffix in &[
            "_alt", "_correct", "_real", "_proper", "_fixed", "_working", "_new", "_better", "_v3",
        ] {
            let fn_name = format!("do_thing{}", suffix);
            let src = format!("fn {}() {{}}\nfn main() {{}}", fn_name);
            let obs = scan_for_dead_alt("example.rs", &src);
            assert_eq!(obs.len(), 1, "expected detection for suffix {}", suffix);
            assert_eq!(obs[0].construct, fn_name);
        }
    }
}
