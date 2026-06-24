use crate::observations::Observation;
use regex::Regex;
use std::sync::OnceLock;

// ── Compiled-once regex statics ───────────────────────────────────────────────

fn ts_ignore_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"//\s*@ts-ignore|//\s*@ts-nocheck|/\*\s*eslint-disable").unwrap())
}

fn as_any_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\bas\s+any\b").unwrap())
}

fn todo_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\bTODO\b|\bFIXME\b|\bunimplemented\b").unwrap())
}

fn claims_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(done|complete|fully\s+covered|production\s+ready|all\s+fixed|victory|fully\s+admitted|victory\s+confirmed)\b").unwrap()
    })
}

fn naming_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(nitro\s*lsp)\b").unwrap())
}

fn vocab_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(GALL|checkpoint|failset|residual|andon|receipt|candidate|blocked|accepted|private\s+doctrine|internal\s+IP)\b").unwrap()
    })
}

// ── Parser ────────────────────────────────────────────────────────────────────

pub fn parse_typescript(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    let is_whitelisted = filepath.contains("diagnostics.ts")
        || filepath.contains("fixtures/")
        || filepath.contains("test/")
        || filepath.contains("tests/");

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;

        if let Some(mat) = ts_ignore_re().find(line) {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_num,
                column: mat.start() + 1,
                kind: "ts_smell".to_string(),
                construct: "ts-ignore".to_string(),
                context: line.to_string(),
                message: "TypeScript ignore or ESLint disable comment detected".to_string(),
            });
        }

        if let Some(mat) = as_any_re().find(line) {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_num,
                column: mat.start() + 1,
                kind: "ts_smell".to_string(),
                construct: "as any".to_string(),
                context: line.to_string(),
                message: "Unsafe type cast 'as any' detected (type laundering)".to_string(),
            });
        }

        if let Some(mat) = todo_re().find(line) {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_num,
                column: mat.start() + 1,
                kind: "ts_smell".to_string(),
                construct: mat.as_str().to_string(),
                context: line.to_string(),
                message: format!(
                    "Unimplemented stub or placeholder '{}' detected",
                    mat.as_str()
                ),
            });
        }

        if let Some(mat) = claims_re().find(line) {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_num,
                column: mat.start() + 1,
                kind: "ts_claim".to_string(),
                construct: mat.as_str().to_string(),
                context: line.to_string(),
                message: format!(
                    "Forbidden claim word '{}' found on TS surface",
                    mat.as_str()
                ),
            });
        }

        if let Some(mat) = naming_re().find(line) {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: mat.start(),
                end_byte: mat.end(),
                line: line_num,
                column: mat.start() + 1,
                kind: "ts_leak".to_string(),
                construct: mat.as_str().to_string(),
                context: line.to_string(),
                message: format!(
                    "Naming Fence violation: Unauthorized name '{}' detected",
                    mat.as_str()
                ),
            });
        }

        if !is_whitelisted {
            if let Some(mat) = vocab_re().find(line) {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: mat.start(),
                    end_byte: mat.end(),
                    line: line_num,
                    column: mat.start() + 1,
                    kind: "ts_leak".to_string(),
                    construct: mat.as_str().to_string(),
                    context: line.to_string(),
                    message: format!(
                        "Scope Fence violation: Leaked internal term '{}'",
                        mat.as_str()
                    ),
                });
            }
        }
    }

    obs
}
