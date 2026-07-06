use std::path::Path;

/// Per-repo configuration for the anti-llm-cheat-lsp scanner.
///
/// Loaded from `anti-llm.toml` in the scanned directory root. All fields
/// default to empty/false so the scanner is fully functional without a config
/// file.
///
/// Example `anti-llm.toml`:
/// ```toml
/// [claim]
/// # Domain terms that are canonical vocabulary, not victory language.
/// # Case-insensitive exact-phrase matches against the observation construct.
/// domain_terms = ["fully admitted", "candidate"]
///
/// [surface]
/// # Path prefixes where SURFACE-001 is informational, not blocking.
/// # Useful for docs/jira/ historical records that reference old library names.
/// non_blocking_path_prefixes = ["docs/jira/", "docs/archive/"]
///
/// [test]
/// # Path prefixes where Vec::contains structural checks are explicitly expected.
/// # The scanner already detects structural vs string checks automatically;
/// # this list suppresses any residual false positives in known paths.
/// structural_check_paths = ["tests/strict_contracts.rs"]
/// ```
use serde::{Deserialize, Serialize};

/// A user-defined forbidden string pattern loaded from `anti.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForbiddenPattern {
    pub name: String,
    pub pattern: String,
    pub files: Option<Vec<String>>,
    pub code: String,
    pub message: String,
    pub blocking: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct AntiLlmConfig {
    #[serde(default)]
    pub claim: ClaimConfig,
    #[serde(default)]
    pub surface: SurfaceConfig,
    #[serde(default)]
    pub test: TestConfig,
    #[serde(default)]
    pub forbidden_string_patterns: Vec<ForbiddenPattern>,
    #[serde(default)]
    pub allowed_suppression_paths: Vec<String>,
    /// Standing-index enforcement (`ANTI-LLM-STANDING-000..006`). `None` when
    /// the repo has not opted in (no `[standing]` table in `anti.toml`) —
    /// in that state the index is never loaded and `ANTI-LLM-STANDING-000`
    /// never fires, matching the default-off convention used by every other
    /// optional section in this file (`domain_terms`, `non_blocking_path_prefixes`,
    /// `structural_check_paths` all default to an empty/inert state).
    #[serde(default)]
    pub standing: Option<StandingConfig>,
}

/// Configuration for CLAIM-004 victory language detection.
#[derive(Debug, Default, Deserialize)]
pub struct ClaimConfig {
    /// Domain terms that are canonical vocabulary in this repo and must not
    /// trigger CLAIM-004. Case-insensitive phrase matches.
    ///
    /// Example: `["fully admitted"]` suppresses the typestate term in a
    /// type-law crate where "fully admitted" means `Evidence<T, Admitted, W>`.
    #[serde(default)]
    pub domain_terms: Vec<String>,
}

/// Configuration for SURFACE-001 (deprecated library) detection.
#[derive(Debug, Default, Deserialize)]
pub struct SurfaceConfig {
    /// Path prefixes (relative to scan root) where SURFACE-001 fires as a
    /// warning rather than an error. Useful for historical doc files and
    /// archived tickets that reference deprecated libraries by name.
    #[serde(default)]
    pub non_blocking_path_prefixes: Vec<String>,
}

/// Configuration for TEST-001 (string-contains assertion) detection.
#[derive(Debug, Default, Deserialize)]
pub struct TestConfig {
    /// Path prefixes where Vec::contains structural checks are expected.
    /// The scanner automatically detects structural vs string-literal checks;
    /// list paths here only when residual false positives remain.
    #[serde(default)]
    pub structural_check_paths: Vec<String>,
}

/// Configuration for `ANTI-LLM-STANDING-000..006` (claim-vs-standing-index
/// enforcement). Only present in `AntiLlmConfig` when the repo's `anti.toml`
/// has a `[standing]` table; absence means the check is off.
#[derive(Debug, Clone, Deserialize)]
pub struct StandingConfig {
    /// Path (relative to the scan root) to the `praxis-standing.v1` JSON
    /// document produced by the standing compiler (e.g. cargo-cicd).
    #[serde(default = "default_standing_index_path")]
    pub index_path: String,
    /// Age, in seconds, after which a loaded index is considered stale
    /// (`ANTI-LLM-STANDING-006`).
    #[serde(default = "default_max_index_age_secs")]
    pub max_index_age_secs: u64,
}

impl Default for StandingConfig {
    fn default() -> Self {
        Self {
            index_path: default_standing_index_path(),
            max_index_age_secs: default_max_index_age_secs(),
        }
    }
}

fn default_standing_index_path() -> String {
    "target/praxis-standing/standing.json".to_string()
}

fn default_max_index_age_secs() -> u64 {
    86_400
}

impl AntiLlmConfig {
    /// Load config from `anti-llm.toml` in `dirpath`. Returns a default
    /// (all-empty) config if the file is absent or unparseable.
    pub fn load_from_dir(dirpath: &str) -> Self {
        // Prefer anti.toml; fall back to anti-llm.toml for backwards compat.
        let config_path = {
            let p = Path::new(dirpath).join("anti.toml");
            if p.is_file() {
                p
            } else {
                Path::new(dirpath).join("anti-llm.toml")
            }
        };
        if !config_path.is_file() {
            return Self::default();
        }
        let content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        star_toml::from_str(&content).unwrap_or_default()
    }

    /// Returns true if `file_path` matches any configured surface non-blocking
    /// prefix. Used by SURFACE-001 to downgrade blocking → warning.
    pub fn surface_is_non_blocking(&self, file_path: &str) -> bool {
        self.surface
            .non_blocking_path_prefixes
            .iter()
            .any(|prefix| file_path.contains(prefix.as_str()))
    }

    /// Returns true if `file_path` matches any configured structural-check
    /// suppression path. Used by TEST-001 to suppress residual false positives.
    pub fn test_is_structural_path(&self, file_path: &str) -> bool {
        self.test.structural_check_paths.iter().any(|prefix| file_path.contains(prefix.as_str()))
    }

    /// Returns true if `file_path` matches any configured allowed suppression paths.
    pub fn is_suppression_allowed(&self, file_path: &str) -> bool {
        self.allowed_suppression_paths.iter().any(|path| file_path.contains(path))
    }
}

#[cfg(test)]
mod standing_config_tests {
    use super::*;

    #[test]
    fn absent_standing_section_deserializes_to_none() {
        let cfg: AntiLlmConfig = star_toml::from_str("").unwrap();
        assert!(cfg.standing.is_none());
    }

    #[test]
    fn empty_standing_table_uses_defaults() {
        let cfg: AntiLlmConfig = star_toml::from_str("[standing]\n").unwrap();
        let s = cfg.standing.expect("[standing] table should deserialize to Some");
        assert_eq!(s.index_path, "target/praxis-standing/standing.json");
        assert_eq!(s.max_index_age_secs, 86_400);
    }

    #[test]
    fn standing_table_with_explicit_index_path() {
        let cfg: AntiLlmConfig =
            star_toml::from_str("[standing]\nindex_path = \"fixtures/standing/standing.json\"\n")
                .unwrap();
        let s = cfg.standing.expect("[standing] table should deserialize to Some");
        assert_eq!(s.index_path, "fixtures/standing/standing.json");
    }
}
