use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;
use regex::Regex;
use std::sync::OnceLock;

fn hardcoded_score_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"fitness\s*[:=]\s*1\.0|fitness\s*[:=]\s*0\.99[0-9]*|\"fitness\"\s*:\s*1\.0"#)
            .unwrap()
    })
}

fn fake_assert_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // assert!(true) — unfalsifiable assert; Rust regex does not support backreferences,
        // so assert_eq!(x, x) identity pattern is not detectable here via regex.
        Regex::new(r"assert!\s*\(\s*true\s*\)").unwrap()
    })
}

fn hardcoded_admitted_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"admitted\s*[:=]\s*true\b|\"admitted\"\s*:\s*true"#).unwrap())
}

/// Scan Rust source line-by-line for fake-alignment patterns.
///
/// Called from `engine::scan_file` for `.rs` files, producing observations
/// whose `context` field is the raw source line. `placeholder::evaluate` then
/// applies the regex detectors against those observations.
pub fn scan_for_fake_alignment(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim_start();

        // Skip comment lines — diagnostics in comments describe patterns, not embody them.
        if trimmed.starts_with("//") {
            continue;
        }
        // Skip string-literal-only lines (heuristic: line starts with a quote after whitespace).
        // Catches message strings that reference pattern names without being actual code.
        if trimmed.starts_with('"') {
            continue;
        }

        let make_obs = |construct: &str| Observation {
            file_path: filepath.to_string(),
            start_byte: 0,
            end_byte: 0,
            line: line_num,
            column: 1,
            kind: "fake_alignment_smell".to_string(),
            construct: construct.to_string(),
            context: line.to_string(),
            message: format!(
                "Fake-alignment pattern '{}' detected on line {}",
                construct, line_num
            ),
        };

        if hardcoded_score_re().is_match(line) {
            obs.push(make_obs("hardcoded_fitness_score"));
        }
        if fake_assert_re().is_match(line) {
            obs.push(make_obs("unfalsifiable_assert"));
        }
        if hardcoded_admitted_re().is_match(line) {
            obs.push(make_obs("hardcoded_admitted"));
        }
    }

    obs
}

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // Only check Rust files — fitness scores in JSONL transcripts are expected.
        if !o.file_path.ends_with(".rs") {
            continue;
        }

        if hardcoded_score_re().is_match(&o.context) {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-PLACEHOLDER-001".to_string(),
                category: "fake_alignment".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Hardcoded fitness=1.0 — fake conformance claim detected".to_string(),
                forbidden_implication: "HardcodedFitness => FakeConformance".to_string(),
                blocking: true,
                required_correction: "Derive fitness from real alignment computation".to_string(),
                required_next_proof: "Show fitness produced by token-replay or alignment algorithm"
                    .to_string(),
            });
        }

        if fake_assert_re().is_match(&o.context) {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-PLACEHOLDER-002".to_string(),
                category: "fake_alignment".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "assert!(true) — unfalsifiable test claim".to_string(),
                forbidden_implication: "AlwaysPassingAssertion => FakeReceipt".to_string(),
                blocking: true,
                required_correction: "Replace with falsifiable assertion on real behavior"
                    .to_string(),
                required_next_proof: "Test must be able to fail on a broken implementation"
                    .to_string(),
            });
        }

        // admitted: true in Rust source (not in JSONL test fixtures) is suspicious.
        if hardcoded_admitted_re().is_match(&o.context) {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-PLACEHOLDER-003".to_string(),
                category: "fake_alignment".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Hardcoded admitted=true — fake admission claim".to_string(),
                forbidden_implication: "HardcodedAdmitted => FakeConformanceVector".to_string(),
                blocking: true,
                required_correction: "Derive admitted from real conformance check result"
                    .to_string(),
                required_next_proof:
                    "admitted must be the output of fitness >= threshold comparison".to_string(),
            });
        }
    }

    diags
}
