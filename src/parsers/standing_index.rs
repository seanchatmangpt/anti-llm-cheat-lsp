//! Parser for `praxis-standing.v1` standing-index documents.
//!
//! The standing index is produced externally (e.g. by the cargo-cicd standing
//! compiler) and consumed here as evidence: markdown/doc claims are checked
//! against it by `rules::standing`. This parser deliberately keeps its own
//! `serde` structs local to this crate rather than depending on a shared
//! `cargo-cicd-core` type — this crate must build and run standalone. Some
//! duplication with the producer's schema is correct, not a smell.
//!
//! The parser is tolerant of a missing or malformed file: `load_standing_index`
//! returns `None` rather than erroring, so a scan of a repo that has not yet
//! adopted standing tracking is unaffected. Callers that *do* expect an index
//! (because `[standing]` is configured in `anti.toml`) are responsible for
//! turning a `None` result into `ANTI-LLM-STANDING-000` — see
//! `rules::standing::evaluate`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The 20 standing statuses defined by `praxis-standing.v1`.
pub const STANDING_STATUSES: &[&str] = &[
    "UNSEEN",
    "DISCOVERED",
    "BUILDS",
    "TESTED",
    "LINT_CLEAN",
    "BENCHMARKED",
    "RECEIPTED",
    "RECEIPT_VERIFIED",
    "OCEL_PROVEN",
    "WASM4PM_PROVEN",
    "CLIENT_VISIBLE",
    "PUBLICATION_READY",
    "PUBLISH_READY",
    "PILOT_READY",
    "PRODUCTION_READY",
    "EXTERNAL_OPERATOR_SIDE_EFFECT",
    "NON_STANDING",
    "QUARANTINED",
    "RETIRED",
    "DUPLICATE",
];

/// Statuses that require a non-empty `scope` string on the artifact per the
/// `praxis-standing.v1` validation rule.
pub const SCOPE_GATED_STATUSES: &[&str] =
    &["PRODUCTION_READY", "PILOT_READY", "PUBLISH_READY", "PUBLICATION_READY"];

/// One piece of evidence backing an artifact's standing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceEntry {
    /// `command | ocel_event | receipt | artifact`.
    pub kind: String,
    /// All other evidence fields (command, `ocel_event` id, receipt digest,
    /// artifact ref, ...). Kept as a flattened JSON map since the shape
    /// varies by `kind` and this parser only needs presence/kind, not the
    /// full per-kind schema.
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

/// One artifact entry in a standing index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingArtifact {
    /// Stable identifier used by the standing compiler.
    pub id: String,
    /// `rust_crate | client | doc | paper | bench | workflow | ontology | binary`.
    pub kind: String,
    /// Path to the artifact, relative to the repo root.
    pub path: String,
    /// Every status the artifact currently carries.
    #[serde(default)]
    pub standing: Vec<String>,
    /// Required non-empty when any scope-gated status is present.
    #[serde(default)]
    pub scope: Option<String>,
    /// Computed readiness-ladder position (0-9).
    #[serde(default)]
    pub ladder_level: Option<u8>,
    /// Evidence backing the artifact's current standing.
    #[serde(default)]
    pub evidence: Vec<EvidenceEntry>,
    /// Records of an actual operator-performed action outside this repo
    /// (e.g. a real publish), distinct from any in-repo `standing` status.
    #[serde(default)]
    pub external_operator_side_effects: Vec<String>,
}

impl StandingArtifact {
    /// True if `status` is present in this artifact's `standing` list.
    pub fn has_status(&self, status: &str) -> bool {
        self.standing.iter().any(|s| s == status)
    }

    /// True if any of `statuses` is present in this artifact's `standing` list.
    pub fn has_any_status(&self, statuses: &[&str]) -> bool {
        statuses.iter().any(|s| self.has_status(s))
    }

    fn requires_scope(&self) -> bool {
        self.standing.iter().any(|s| SCOPE_GATED_STATUSES.contains(&s.as_str()))
    }

    fn has_valid_scope(&self) -> bool {
        self.scope.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false)
    }
}

/// Top-level `praxis-standing.v1` document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingIndex {
    /// Release this index was generated for.
    pub release_id: String,
    /// RFC 3339 UTC timestamp of generation (checked by `ANTI-LLM-STANDING-006`).
    pub generated_at_utc: String,
    /// Name of the tool that produced this index (e.g. the cargo-cicd
    /// standing compiler).
    pub generator: String,
    /// Schema version; `"1"` for `praxis-standing.v1`.
    pub standing_version: String,
    /// The artifacts tracked by this index.
    #[serde(default)]
    pub artifacts: Vec<StandingArtifact>,
}

impl StandingIndex {
    /// Find an artifact by id, path, or a fuzzy path/id containment match —
    /// markdown claims usually reference a crate name or path fragment, not
    /// the exact `id` string used by the standing compiler.
    pub fn find_artifact(&self, hint: &str) -> Option<&StandingArtifact> {
        let hint = hint.trim();
        if hint.is_empty() {
            return None;
        }
        self.artifacts.iter().find(|a| {
            a.id == hint
                || a.path == hint
                || a.path.ends_with(hint)
                || a.id.contains(hint)
                || hint.contains(a.id.as_str())
        })
    }

    /// Validate the scope-gating rule from `praxis-standing.v1`: any artifact
    /// carrying a `PRODUCTION_READY`/`PILOT_READY`/`PUBLISH_READY`/
    /// `PUBLICATION_READY` status must have a non-empty `scope`. Returns one
    /// message per invalid artifact; empty when the index is fully valid.
    pub fn validate(&self) -> Vec<String> {
        self.artifacts
            .iter()
            .filter(|a| a.requires_scope() && !a.has_valid_scope())
            .map(|a| {
                format!(
                    "artifact '{}' carries a scope-gated status ({:?}) without a non-empty scope",
                    a.id,
                    a.standing
                        .iter()
                        .filter(|s| SCOPE_GATED_STATUSES.contains(&s.as_str()))
                        .collect::<Vec<_>>()
                )
            })
            .collect()
    }
}

/// Load and deserialize a standing index from `path`. Tolerant of a missing
/// or malformed file — returns `None` in both cases rather than erroring, so
/// a directory scan never fails outright for lack of standing evidence. The
/// `ANTI-LLM-STANDING-000` diagnostic is how a configured-but-absent index is
/// surfaced instead.
pub fn load_standing_index(path: &Path) -> Option<StandingIndex> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_none() {
        assert!(load_standing_index(Path::new("does/not/exist.json")).is_none());
    }

    #[test]
    fn malformed_json_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("standing.json");
        std::fs::write(&path, b"{ not json").unwrap();
        assert!(load_standing_index(&path).is_none());
    }

    #[test]
    fn well_formed_index_parses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("standing.json");
        std::fs::write(
            &path,
            r#"{
                "release_id": "v0.0.1",
                "generated_at_utc": "2026-01-01T00:00:00Z",
                "generator": "test",
                "standing_version": "1",
                "artifacts": [
                    {"id": "demo", "kind": "rust_crate", "path": "src/demo.rs", "standing": ["DISCOVERED"]}
                ]
            }"#,
        )
        .unwrap();
        let idx = load_standing_index(&path).expect("should parse");
        assert_eq!(idx.artifacts.len(), 1);
        assert!(idx.find_artifact("demo").is_some());
        assert!(idx.validate().is_empty());
    }

    #[test]
    fn scope_gated_status_without_scope_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("standing.json");
        std::fs::write(
            &path,
            r#"{
                "release_id": "v0.0.1",
                "generated_at_utc": "2026-01-01T00:00:00Z",
                "generator": "test",
                "standing_version": "1",
                "artifacts": [
                    {"id": "demo", "kind": "rust_crate", "path": "src/demo.rs", "standing": ["PRODUCTION_READY"]}
                ]
            }"#,
        )
        .unwrap();
        let idx = load_standing_index(&path).expect("should parse");
        assert_eq!(idx.validate().len(), 1);
    }
}
