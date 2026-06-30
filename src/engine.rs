use std::{fs, path::Path, sync::OnceLock};

use aho_corasick::AhoCorasick;
use regex::Regex;

use crate::{
    config::{AntiLlmConfig, ForbiddenPattern},
    diagnostics::AntiLlmDiagnostic,
    observations::Observation,
    parsers::{
        cargo_lock, cargo_toml, contract, fitness_report, ggen_toml, json_rpc, markdown_claims,
        receipt_json, refgraph, rust_tree_sitter, tera_template, typescript, typescript_ast,
    },
    rules::{
        authority, claims, complexity, contract as contract_rules, dead_alt, declare_laws,
        determinism, ggen, hedge, hollow, lsp318, mutation, ocel_rules, oracle, placeholder,
        receipts, refgraph as refgraph_rules, routes, rust_smells, surface, test, trace,
        typescript as ts_rules, typescript_ast as ts_ast_rules, version,
    },
};

// ── Line index — O(n) build, O(log n) lookup ──────────────────────────────────

fn build_line_index(content: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(content.len() / 40 + 1);
    offsets.push(0); // line 1 starts at byte 0
    for pos in memchr::memchr_iter(b'\n', content) {
        offsets.push(pos + 1); // line N+1 starts after the newline
    }
    offsets
}

fn byte_to_line(line_index: &[usize], byte_offset: usize) -> usize {
    match line_index.partition_point(|&start| start <= byte_offset) {
        0 => 1,
        n => n,
    }
}

// ── Raw-smell automaton (compiled once) ───────────────────────────────────────
//
// Victory-language terms are intentionally absent here. They are owned by
// `rules::claims::VICTORY_TERMS` and detected by a separate pass so that
// per-repo domain-term exemptions can be applied before emitting diagnostics.

const RAW_SMELL_PATTERNS: &[&str] = &[
    "tower-lsp",                                            // 0 — needs lsp-max suffix check
    "tower_lsp",                                            // 1 — needs lsp-max suffix check
    "CLAP",                                                 // 2
    "Routing to PackPlan",                                  // 3
    "test result: ok",                                      // 4
    "v1.0.0",                                               // 5
    "version = \"1.0.0\"",                                  // 6
    "CLAP-DEBUG",                                           // 7
    "CLAP-DEBUG-PATH",                                      // 8
    "Content was:",                                         // 9
    "Path was:",                                            // 10
    "static scan as route proof", // 11 (before "static scan" — LeftmostLongest)
    "static scan",                // 12
    "route proof",                // 13
    "ChangelogCoverage(15 rows) => SpecCoverage(LSP 3.18)", // 14
    "ChangelogCoverage(15 rows) \u{21d2} SpecCoverage(LSP 3.18)", // 15
    "15-row changelog matrix is being treated as full LSP 3.18 combinatorial coverage", // 16
    "ANTI-LLM-OCEL-001-TRIGGER",  // 17
    "ANTI-LLM-OCEL-002-TRIGGER",  // 18
    "\"bypassed_compat\": true",  // 19
    "use wasm4pm::",              // 20
    // ANTI-LLM-OCEL-006: bypass-comment smells — intent to skip the compat boundary
    "// direct wasm4pm",          // 21
    "// bypass compat",           // 22
    "// skip compat",             // 23
    "// compat not needed",       // 24
    "// wasm4pm directly",        // 25
    "// direct use of wasm4pm",   // 26
];

fn raw_smell_ac() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| {
        aho_corasick::AhoCorasickBuilder::new()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build(RAW_SMELL_PATTERNS)
            .unwrap()
    })
}

// ── TEST-001 helper — classify .contains() receiver ──────────────────────────

/// Classify a test-file line that contains both `assert` and `.contains`.
///
/// Returns the `construct` string for the resulting observation:
///
/// - `"assert_contains_string"` — argument is a string literal, e.g.
///   `assert!(x.to_string().contains("VariantName"))`. This is the real cheat:
///   the test couples to the Display representation instead of the type.
///
/// - `"assert_contains_structural"` — argument is a reference or enum path,
///   e.g. `assert!(vec.contains(&Enum::Variant))`. This is structural equality
///   via `PartialEq` — acceptable.
///
/// - `"assert_contains"` — receiver cannot be classified from the line text.
///   Flagged conservatively as a potential cheat.
///
/// Extract the integer literal from an `assert_eq!(expr.len(), N)` or
/// `assert_eq!(N, expr.len())` call. Returns `None` when no literal is found
/// or the literal cannot be parsed as `u64`.
fn extract_len_literal(line: &str) -> Option<u64> {
    // Scan every whitespace-delimited token; return the first that parses as u64
    // and is adjacent to ".len()" in the line (within 30 chars).
    let len_pos = line.find(".len()")?;
    // Look in the 60-char window around the .len() call
    let start = len_pos.saturating_sub(0);
    let end = (len_pos + 30).min(line.len());
    let window = &line[start..end];
    for token in window.split(|c: char| !c.is_ascii_digit()) {
        if token.is_empty() {
            continue;
        }
        if let Ok(n) = token.parse::<u64>() {
            if n > 0 {
                return Some(n);
            }
        }
    }
    // Also scan the full line for a bare integer after the comma
    for token in line.split(|c: char| !c.is_ascii_digit()) {
        if token.is_empty() {
            continue;
        }
        if let Ok(n) = token.parse::<u64>() {
            if n > 0 && n <= 100 {
                return Some(n);
            }
        }
    }
    None
}

fn classify_contains(line: &str) -> &'static str {
    // Find the `.contains(` token to examine what immediately follows the `(`.
    let Some(pos) = line.find(".contains(") else {
        return "assert_contains";
    };
    let after = line[pos + ".contains(".len()..].trim_start();

    if after.starts_with('"') || after.starts_with("r\"") || after.starts_with("r#\"") {
        // String literal argument → Display / output cheat
        "assert_contains_string"
    } else if after.starts_with('&') || after.starts_with("&&") {
        // Reference argument → structural PartialEq check (Vec::contains(&T))
        "assert_contains_structural"
    } else if after.starts_with("format!") || after.starts_with("&format!") {
        // format!() argument → the string is constructed then searched → cheat
        "assert_contains_string"
    } else {
        // Cannot classify — flag conservatively
        "assert_contains"
    }
}

// ── File scanner ──────────────────────────────────────────────────────────────

pub fn scan_file(filepath: &str) -> Vec<Observation> {
    let mut obs = Vec::new();
    let path = Path::new(filepath);
    if !path.is_file() {
        return obs;
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return obs,
    };

    let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or_default();

    // Skip self-references (engine.rs / lsp318.rs define some of these strings as data)
    let is_self_excluded = filepath.ends_with("src/rules/lsp318.rs")
        || filepath.ends_with("src/engine.rs")
        || filepath.ends_with("rules/lsp318.rs")
        || filepath.ends_with("engine.rs");

    // Rule and parser source files define cheat-pattern strings as data — exclude them
    // from the hollow/placeholder line scans to prevent self-detection.
    let is_rule_or_parser_src =
        filepath.contains("src/rules/") || filepath.contains("src/parsers/");

    // 1. Raw text scan — single AhoCorasick pass over entire file
    if !is_self_excluded {
        let line_index = build_line_index(content.as_bytes());

        for mat in raw_smell_ac().find_iter(&content) {
            let pattern_idx = mat.pattern().as_usize();
            let smell = RAW_SMELL_PATTERNS[pattern_idx];
            let idx = mat.start();

            // tower-lsp / tower_lsp: skip lsp-max suffixed variants
            if pattern_idx == 0 || pattern_idx == 1 {
                let suffix = &content[idx + smell.len()..];
                if suffix.starts_with("-max")
                    || suffix.starts_with("_max")
                    || suffix.starts_with("::max")
                {
                    continue;
                }
            }

            let line_count = byte_to_line(&line_index, idx);
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: idx,
                end_byte: idx + smell.len(),
                line: line_count,
                column: 1,
                kind: "raw_text".to_string(),
                construct: smell.to_string(),
                context: smell.to_string(),
                message: format!("Raw text pattern '{}' detected", smell),
            });
        }
    }

    // 2. Victory-language scan (delegated to claims rule vocabulary)
    //    Domain-term exemptions are applied later in evaluate_diagnostics.
    if !is_self_excluded {
        // Pass empty domain_terms — exemptions apply at evaluate time.
        obs.extend(claims::scan_for_victory(filepath, &content, "raw_text", &[]));
    }

    // 3. Test-file checks
    let is_test_file = filepath.contains("tests/")
        || filepath.ends_with("_test.rs")
        || filepath.contains("/test/");
    if is_test_file {
        for (line_idx, line) in content.lines().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            // TEST-001: .contains("string literal") assertion
            if line.contains("assert") && line.contains(".contains") {
                let construct = classify_contains(line);
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "test_smell".to_string(),
                    construct: construct.to_string(),
                    context: line.to_string(),
                    message: format!(
                        ".contains() assertion classified as '{}' in test file",
                        construct
                    ),
                });
            }

            // TEST-002: assert!(expr.is_ok()) with no subsequent value extraction.
            // Proves absence of panic, not correctness of return value.
            // Heuristic: the assertion is on its own line and ends with .is_ok())
            // (single expression — no chained .unwrap(), .map(), etc.).
            if trimmed.starts_with("assert!(")
                && trimmed.contains(".is_ok())")
                && !trimmed.contains(".unwrap()")
                && !trimmed.contains(".map(")
                && !trimmed.contains(".and_then(")
                && !trimmed.contains("unwrap_or")
            {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "test_smell".to_string(),
                    construct: "assert_is_ok_only".to_string(),
                    context: line.to_string(),
                    message: "assert!(expr.is_ok()) verifies no panic but not the return value \
                        — couple with a structural check on the Ok payload or engine state"
                        .to_string(),
                });
            }

            // TEST-004: error-swallowing test helper — unwrap_or_default() /
            // unwrap_or("") on a call expression whose receiver is the
            // system under test (not a library utility).
            // Pattern: any .unwrap_or_default() or .unwrap_or( in a test file
            // where the receiver is a method call (indicates SUT result discarded).
            if (trimmed.contains(".unwrap_or_default()") || trimmed.contains(".unwrap_or("))
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("let _ =")
            {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "test_smell".to_string(),
                    construct: "test_unwrap_or_swallow".to_string(),
                    context: line.to_string(),
                    message: "unwrap_or_default() / unwrap_or() in test code silently converts \
                        SUT errors into empty/default values — failures pass undetected"
                        .to_string(),
                });
            }

            // TEST-005: count-match assertion — assert_eq!(X.len(), N) where N
            // is a small integer literal (≤ 100). Confirms quantity not quality;
            // especially suspicious when N matches the number in a "write N
            // tests" prompt. The existing TRACE-002 rule covers trace.len()
            // specifically; this rule generalises to all .len() in test assertions.
            if (trimmed.starts_with("assert_eq!(") || trimmed.starts_with("assert_eq! ("))
                && trimmed.contains(".len()")
            {
                if let Some(count) = extract_len_literal(trimmed) {
                    if count <= 100 {
                        obs.push(Observation {
                            file_path: filepath.to_string(),
                            start_byte: 0,
                            end_byte: 0,
                            line: line_num,
                            column: 1,
                            kind: "test_smell".to_string(),
                            construct: format!("assert_len_literal_{}", count),
                            context: line.to_string(),
                            message: format!(
                                "assert_eq!(...len(), {count}) verifies quantity not quality — \
                                combine with at least one structural element check"
                            ),
                        });
                    }
                }
            }

            if !filepath.contains("dogfood.rs") && line.contains("negative_controls") {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "test_smell".to_string(),
                    construct: "negative_control_reference".to_string(),
                    context: line.to_string(),
                    message: "Standard test references negative controls directory".to_string(),
                });
            }

            // TEST-006: snapshot-as-spec — assert_snapshot! / assert_yaml_snapshot!
            // with no assert_eq!/assert_ne! on the same binding creates circular specs
            // where LLM output becomes the expected value.
            if trimmed.contains("assert_snapshot!(") || trimmed.contains("assert_yaml_snapshot!(")
            {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "test_smell".to_string(),
                    construct: "assert_snapshot_no_eq".to_string(),
                    context: line.to_string(),
                    message: "assert_snapshot!/assert_yaml_snapshot! without structural \
                        assert_eq!/assert_ne! creates a circular spec — LLM output \
                        becomes the expected value"
                        .to_string(),
                });
            }
        }
    }

    // 4. Type-specific parsers
    if filename == "Cargo.toml" {
        obs.extend(cargo_toml::parse_cargo_toml(filepath, &content));
    } else if filename == "Cargo.lock" {
        obs.extend(cargo_lock::parse_cargo_lock(filepath, &content));
    } else if filename.ends_with(".rs") {
        obs.extend(rust_tree_sitter::parse_rust_ast(filepath, &content));
        if !is_rule_or_parser_src {
            obs.extend(hollow::scan_for_hollow(filepath, &content));
            obs.extend(placeholder::scan_for_fake_alignment(filepath, &content));
        }
        obs.extend(dead_alt::scan_for_dead_alt(filepath, &content));

        // STRANGE-008: println! in non-test production .rs code (debug output left in)
        // STRANGE-009: #[cfg(test)] wrapping production logic (law evasion)
        if !is_self_excluded && !is_rule_or_parser_src && !is_test_file {
            for (line_idx, line) in content.lines().enumerate() {
                let line_num = line_idx + 1;
                let trimmed = line.trim();
                if trimmed.contains("println!(") && !trimmed.starts_with("//") {
                    obs.push(Observation {
                        file_path: filepath.to_string(),
                        start_byte: 0,
                        end_byte: 0,
                        line: line_num,
                        column: 1,
                        kind: "rust_strange".to_string(),
                        construct: "println_in_production".to_string(),
                        context: line.to_string(),
                        message: format!("println! on line {} in production code", line_num),
                    });
                }
                if trimmed.starts_with("#[cfg(test)]") {
                    obs.push(Observation {
                        file_path: filepath.to_string(),
                        start_byte: 0,
                        end_byte: 0,
                        line: line_num,
                        column: 1,
                        kind: "rust_strange".to_string(),
                        construct: "cfg_test_in_production".to_string(),
                        context: line.to_string(),
                        message: format!("#[cfg(test)] on line {} outside a test file", line_num),
                    });
                }
            }
        }

        // ORACLE-007 / DECLARE-006: detect .unknown.clear() and
        // .unknown = HashSet::new() on ConformanceVector-shaped structs.
        // These patterns collapse Unknown to empty — a DECLARE-006 violation.
        if !is_self_excluded {
            let line_index = build_line_index(content.as_bytes());
            for (line_idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                let is_unknown_collapse = trimmed.contains(".unknown.clear()")
                    || (trimmed.contains(".unknown") && trimmed.contains("= HashSet::new()"))
                    || (trimmed.contains(".unknown") && trimmed.contains("= Default::default()"));
                if is_unknown_collapse {
                    let byte_off = content
                        .lines()
                        .take(line_idx)
                        .map(|l| l.len() + 1)
                        .sum::<usize>();
                    let _ = byte_to_line(&line_index, byte_off); // keep index warm
                    obs.push(Observation {
                        file_path: filepath.to_string(),
                        start_byte: byte_off,
                        end_byte: byte_off + line.len(),
                        line: line_idx + 1,
                        column: 1,
                        kind: "ast_node".to_string(),
                        construct: "unknown_axis_cleared".to_string(),
                        context: trimmed.chars().take(120).collect(),
                        message: format!(
                            "ConformanceVector.unknown cleared at line {} — Unknown must not \
                             be silently collapsed to empty",
                            line_idx + 1
                        ),
                    });
                }
            }
        }
    } else if filename.ends_with(".md") {
        obs.extend(markdown_claims::parse_markdown_claims(filepath, &content));
    } else if filename.ends_with(".json") || filename.ends_with(".jsonl") {
        if filepath.contains("transcripts") {
            obs.extend(json_rpc::parse_json_rpc_transcript(filepath, &content));
        } else if filepath.contains("receipts") {
            obs.extend(receipt_json::parse_receipt_json(filepath, &content));
        }
    } else if filename.ends_with(".ts")
        || filename.ends_with(".tsx")
        || filename.ends_with(".js")
        || filename.ends_with(".jsx")
        || filename.ends_with(".mts")
        || filename.ends_with(".mjs")
        || filename.ends_with(".cts")
        || filename.ends_with(".cjs")
    {
        obs.extend(typescript::parse_typescript(filepath, &content));
        obs.extend(typescript_ast::parse_typescript_ast(filepath, &content));
        obs.extend(hollow::scan_for_hollow_ts(filepath, &content));
    } else if filename == "ggen.toml" {
        obs.extend(ggen_toml::parse_ggen_toml(filepath, &content));
    } else if filename.ends_with(".tera") {
        obs.extend(tera_template::parse_tera_template(filepath, &content));
        obs.extend(hollow::scan_for_hollow_tera(filepath, &content));
    } else if filename.ends_with(".json")
        && (filepath.contains("ocel/reports") || filepath.contains("fitness_reports"))
    {
        obs.extend(fitness_report::parse_fitness_report(filepath, &content));
    }

    // hedge runs on all file types (catches hedge comments in any language)
    obs.extend(hedge::scan_for_hedge(filepath, &content));

    // VERSION-004 / VERSION-005: CalVer range and placeholder scan — runs on
    // all text files (Cargo.toml, .rs, .md, .json, etc.) because LLM-generated
    // placeholder versions can appear in any file type.
    if !is_self_excluded {
        obs.extend(version::scan_for_calver_violations(filepath, &content));
    }

    obs
}

// ── Config-pattern scanner ─────────────────────────────────────────────────────

/// Returns true if `filepath` matches any of the glob patterns in `files`.
/// Patterns are simple glob suffixes (e.g. "*.ts", "*.tsx"). If `files` is
/// None the pattern applies to all files.
fn file_matches_patterns(filepath: &str, files: &Option<Vec<String>>) -> bool {
    let Some(patterns) = files else { return true };
    patterns.iter().any(|pat| {
        // Simple glob: "*.ts" → filepath ends with ".ts"
        if let Some(suffix) = pat.strip_prefix("*.") {
            filepath.ends_with(&format!(".{suffix}"))
        } else {
            filepath.contains(pat.as_str())
        }
    })
}

/// Apply all `forbidden_string_patterns` from config against `content` and
/// emit `config_pattern` observations for every match.
pub fn apply_config_patterns(
    filepath: &str,
    content: &str,
    patterns: &[ForbiddenPattern],
) -> Vec<Observation> {
    let mut obs = Vec::new();
    let line_index = build_line_index(content.as_bytes());

    for fp in patterns {
        if !file_matches_patterns(filepath, &fp.files) {
            continue;
        }
        let re = match Regex::new(&fp.pattern) {
            Ok(r) => r,
            Err(_) => continue, // skip invalid patterns
        };
        for mat in re.find_iter(content) {
            let line = byte_to_line(&line_index, mat.start());
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line,
                column: 1,
                kind: "config_pattern".to_string(),
                construct: fp.name.clone(),
                context: mat.as_str().chars().take(120).collect(),
                message: fp.message.clone(),
            });
        }
    }

    obs
}

// ── Directory scanner — ignore::Walk respects .gitignore ─────────────────────

pub fn scan_directory(dirpath: &str) -> Vec<Observation> {
    let config = AntiLlmConfig::load_from_dir(dirpath);
    scan_directory_with_config(dirpath, &config)
}

pub fn scan_directory_with_config(dirpath: &str, config: &AntiLlmConfig) -> Vec<Observation> {
    let mut obs = Vec::new();
    let path = Path::new(dirpath);
    if !path.is_dir() {
        return obs;
    }

    let walker = ignore::WalkBuilder::new(path)
        .hidden(false)
        .add_custom_ignore_filename(".anti-llm-ignore")
        .filter_entry(|e| e.file_name().to_string_lossy() != "fixtures")
        .build();

    for entry in walker.flatten() {
        if entry.path().is_file() {
            let fp = entry.path().to_string_lossy();
            obs.extend(scan_file(&fp));
            // Apply config forbidden_string_patterns
            if !config.forbidden_string_patterns.is_empty() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    obs.extend(apply_config_patterns(
                        &fp,
                        &content,
                        &config.forbidden_string_patterns,
                    ));
                }
            }
        }
    }

    // GGEN-YIELD-004: cross-file competing-authority detection across all ggen.toml files
    obs.extend(ggen_toml::detect_competing_authority(&obs.clone()));
    // CONTRACT-001/002: cross-file vocabulary schism detection
    obs.extend(contract::detect_contract_schism(&obs.clone()));
    // REFGRAPH-001: bounded transitive failset over the reference closure
    obs.extend(refgraph::detect_transitive_failset(&obs.clone()));

    obs
}

/// Evaluate diagnostics with a default (all-empty) config.
///
/// Suitable for programmatic callers that do not have a scan directory.
/// Callers with a directory should prefer `evaluate_diagnostics_with_config`.
pub fn evaluate_diagnostics(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    evaluate_diagnostics_with_config(obs, &AntiLlmConfig::default())
}

/// Evaluate diagnostics using a per-repo config loaded from `anti-llm.toml`.
pub fn evaluate_diagnostics_with_config(
    obs: &[Observation],
    config: &AntiLlmConfig,
) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    diags.extend(surface::evaluate(obs, config));
    diags.extend(authority::evaluate(obs));
    diags.extend(receipts::evaluate(obs));
    diags.extend(routes::evaluate(obs));
    diags.extend(mutation::evaluate(obs, config));
    diags.extend(version::evaluate(obs, config));
    diags.extend(test::evaluate(obs, config));
    diags.extend(rust_smells::evaluate(obs));
    diags.extend(determinism::evaluate(obs, config));
    diags.extend(lsp318::evaluate(obs));
    diags.extend(ocel_rules::evaluate(obs));
    diags.extend(ts_rules::evaluate(obs));
    diags.extend(ts_ast_rules::evaluate(obs));
    diags.extend(ggen::evaluate(obs));
    diags.extend(complexity::evaluate(obs));
    diags.extend(oracle::evaluate(obs, config));
    diags.extend(trace::evaluate(obs));
    diags.extend(contract_rules::evaluate(obs));
    diags.extend(refgraph_rules::evaluate(obs));
    diags.extend(declare_laws::evaluate(obs));
    diags.extend(hollow::evaluate(obs, config));
    diags.extend(placeholder::evaluate(obs, config));
    diags.extend(hedge::evaluate(obs, config));
    diags.extend(dead_alt::evaluate(obs, config));

    let has_non_victory_errors = diags.iter().any(|d| d.code != "ANTI-LLM-CLAIM-004");
    diags.extend(claims::evaluate(obs, &config.claim.domain_terms, has_non_victory_errors));

    // Config-driven forbidden_string_patterns → diagnostics
    for o in obs.iter().filter(|o| o.kind == "config_pattern") {
        // Look up the pattern definition by construct name to get blocking flag and code/message
        let fp_def = config.forbidden_string_patterns.iter().find(|fp| fp.name == o.construct);
        let (code, message, blocking) = if let Some(fp) = fp_def {
            (fp.code.clone(), fp.message.clone(), fp.blocking.unwrap_or(true))
        } else {
            ("ANTI-LLM-CONFIG-001".to_string(), o.message.clone(), true)
        };
        diags.push(AntiLlmDiagnostic {
            code,
            category: "config-pattern".to_string(),
            file_path: o.file_path.clone(),
            line: o.line,
            column: o.column,
            message: format!("[CONFIG] {}", message),
            forbidden_implication: format!("ConfigPattern({}) => Blocked", o.construct),
            blocking,
            required_correction: "Remove or replace the forbidden pattern per anti.toml"
                .to_string(),
            required_next_proof: "Verify pattern no longer appears in matched files".to_string(),
        });
    }

    // Deduplicate by (file_path, line, code)
    let mut seen = std::collections::HashSet::new();
    diags.retain(|d| seen.insert((d.file_path.clone(), d.line, d.code.clone())));

    diags
}
