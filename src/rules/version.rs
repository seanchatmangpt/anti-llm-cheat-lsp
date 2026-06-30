//! CalVer version law (`ANTI-LLM-VERSION-*`): the workspace version must be
//! `YY.M.D`, not SemVer. For the rationale and a runnable witness that validates
//! the live `CARGO_PKG_VERSION` and rejects SemVer-shaped strings, see
//! `examples/calver_law_explained.rs` (`cargo run --example calver_law_explained`).

use std::sync::OnceLock;

use regex::Regex;

use crate::{diagnostics::AntiLlmDiagnostic, observations::Observation};

fn calver_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(\d{2})\.(\d{1,2})\.(\d{1,2})\b").expect("calver regex"))
}

/// Scan file content for CalVer-shaped strings with out-of-range or placeholder
/// components. Produces observations consumed by `evaluate` → VERSION-004 /
/// VERSION-005.
pub fn scan_for_calver_violations(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        for cap in calver_re().captures_iter(line) {
            let year: u32 = cap[1].parse().unwrap_or(0);
            let month: u32 = cap[2].parse().unwrap_or(0);
            let day: u32 = cap[3].parse().unwrap_or(0);
            let matched = cap[0].to_string();

            // VERSION-004: component values outside plausible CalVer ranges
            if year < 25 || year > 30 || month > 12 || day > 31 {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "calver_scan".to_string(),
                    construct: "calver_out_of_range".to_string(),
                    context: format!("{matched} (year={year}, month={month}, day={day})"),
                    message: format!(
                        "CalVer component out of valid range in '{}': year={year}, month={month}, day={day}",
                        matched
                    ),
                });
            }

            // VERSION-005: X.1.1 placeholder or zero component
            let is_jan_first = month == 1 && day == 1;
            let has_zero_component = month == 0 || day == 0;
            if is_jan_first || has_zero_component {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_num,
                    column: 1,
                    kind: "calver_scan".to_string(),
                    construct: "calver_placeholder".to_string(),
                    context: format!("{matched} (year={year}, month={month}, day={day})"),
                    message: format!(
                        "Static CalVer placeholder in '{}': Jan-1 default or zero component",
                        matched
                    ),
                });
            }
        }
    }

    obs
}

pub fn evaluate(
    obs: &[Observation],
    config: &crate::config::AntiLlmConfig,
) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // v1.0.0 or version = "1.0.0" found
        if (o.construct == "version = \"1.0.0\""
            || o.context.contains("v1.0.0")
            || o.context.contains("1.0.0"))
            && !config.is_suppression_allowed(&o.file_path)
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-VERSION-001".to_string(),
                category: "version".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Default template version '1.0.0' or 'v1.0.0' found in project configuration.".to_string(),
                forbidden_implication: "Template default => release law".to_string(),
                blocking: true,
                required_correction: "Specify CalVer version (e.g. v26.6.5) instead of standard v1.0.0 template version.".to_string(),
                required_next_proof: "Check project Cargo.toml metadata.".to_string(),
            });
        }

        // PATH-DEP with explicit non-CalVer version
        if o.construct == "path_dep_with_semver_version"
            && !config.is_suppression_allowed(&o.file_path)
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-VERSION-002".to_string(),
                category: "version".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Path dependency declares explicit SemVer version; omit version field or use CalVer".to_string(),
                forbidden_implication: "Path dep version pin => calver law".to_string(),
                blocking: false,
                required_correction: "Remove the version field from the path dependency or replace with a CalVer string (YY.M.D).".to_string(),
                required_next_proof: "Check path dependency declarations in Cargo.toml.".to_string(),
            });
        }

        // [workspace.package] with non-CalVer version
        if o.construct == "workspace_semver_version" && !config.is_suppression_allowed(&o.file_path)
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-VERSION-003".to_string(),
                category: "version".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message:
                    "Workspace root declares SemVer version; workspace must use CalVer (YY.M.D)"
                        .to_string(),
                forbidden_implication: "Workspace semver => calver law".to_string(),
                blocking: false,
                required_correction: "Replace workspace version with CalVer (e.g. 26.6.12)."
                    .to_string(),
                required_next_proof: "Check [workspace.package] version in root Cargo.toml."
                    .to_string(),
            });
        }

        // VERSION-004: CalVer-shaped string with out-of-range component values
        if o.construct == "calver_out_of_range" && !config.is_suppression_allowed(&o.file_path) {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-VERSION-004".to_string(),
                category: "version".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!(
                    "CalVer component out of valid range: {}",
                    o.context
                ),
                forbidden_implication: "CalVerShaped => ValidCalVerRange".to_string(),
                blocking: true,
                required_correction:
                    "Use a valid CalVer string (YY.M.D where 25<=YY<=30, 1<=M<=12, 1<=D<=31)."
                        .to_string(),
                required_next_proof:
                    "Verify the version string reflects an actual calendar date in range."
                        .to_string(),
            });
        }

        // VERSION-005: Static CalVer placeholder (X.1.1 / zero month or day)
        if o.construct == "calver_placeholder" && !config.is_suppression_allowed(&o.file_path) {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-VERSION-005".to_string(),
                category: "version".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: format!(
                    "Static CalVer placeholder detected: {}",
                    o.context
                ),
                forbidden_implication: "CalVerPlaceholder => MeasuredCalVer".to_string(),
                blocking: true,
                required_correction:
                    "Replace the placeholder CalVer with the actual release date (e.g. 26.6.30)."
                        .to_string(),
                required_next_proof:
                    "Confirm the version encodes a real sprint/release date, not a default."
                        .to_string(),
            });
        }
    }

    diags
}
