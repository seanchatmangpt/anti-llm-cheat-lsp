//! Claim-vs-standing-index enforcement (`ANTI-LLM-STANDING-000..006`).
//!
//! Markdown/doc claims captured by `parsers::markdown_claims` as
//! `standing_claim` observations are checked against a `praxis-standing.v1`
//! index (see `parsers::standing_index`). The index itself is loaded once per
//! scan by `engine::evaluate_diagnostics_with_config` and threaded through
//! here, mirroring how `ocel_rules` consumes pre-computed observations rather
//! than re-parsing evidence per rule.
//!
//! `config.standing` gates the whole subsystem: `None` (no `[standing]`
//! table in `anti.toml`) means this repo has not opted into standing
//! tracking, so `ANTI-LLM-STANDING-000/002/003/004/005/006` never fire —
//! matching the "default off, opt in" convention every other optional
//! section in `config.rs` already uses (`domain_terms`,
//! `non_blocking_path_prefixes`, `structural_check_paths`). `001`
//! (unscoped-claim) is the one exception: it is a purely textual check on
//! the claim itself and does not need an index to evaluate, so it always
//! runs.
//!
//! Path-based exemptions reuse `config.surface_is_non_blocking` (the
//! mechanism `ANTI-LLM-SURFACE-001` already uses to downgrade historical/doc
//! references to non-blocking) rather than inventing a parallel
//! `[standing]`-scoped exemption list.

use chrono::{DateTime, Utc};

use crate::{
    config::AntiLlmConfig, diagnostics::AntiLlmDiagnostic, observations::Observation,
    parsers::standing_index::StandingIndex,
};

/// Claim kinds gated by `ANTI-LLM-STANDING-001` (must carry an explicit scope).
const READY_CLAIM_KINDS: &[&str] =
    &["production-ready", "pilot-ready", "publish-ready", "publication-ready"];

/// `claim_kind` -> the standing status it directly asserts, for the generic
/// `ANTI-LLM-STANDING-002` comparison. `alive`/`verified` (004), `benchmarked`
/// (005), and `published` (003) have their own more specific rules below and
/// are deliberately excluded here so one claim doesn't double-fire two codes.
fn direct_status_for(claim_kind: &str) -> Option<&'static str> {
    match claim_kind {
        "production-ready" => Some("PRODUCTION_READY"),
        "pilot-ready" => Some("PILOT_READY"),
        "publish-ready" => Some("PUBLISH_READY"),
        "publication-ready" => Some("PUBLICATION_READY"),
        "ocel-proven" => Some("OCEL_PROVEN"),
        "wasm4pm-proven" => Some("WASM4PM_PROVEN"),
        _ => None,
    }
}

/// Pull one `key=value` field out of the `key=value, key=value, ...` context
/// string produced by `markdown_claims::scan_for_standing_claims` (the same
/// flat-context convention `parsers::fitness_report` uses).
fn extract_field(context: &str, key: &str, next_key: &str) -> String {
    let marker = format!("{key}=");
    let Some(start) = context.find(&marker) else {
        return String::new();
    };
    let rest = &context[start + marker.len()..];
    let next_marker = format!(", {next_key}=");
    let end = rest.find(&next_marker).unwrap_or(rest.len());
    rest[..end].trim().to_string()
}

fn scope_of(o: &Observation) -> Option<String> {
    match extract_field(&o.context, "scope", "artifact") {
        v if v.is_empty() || v == "NONE" => None,
        v => Some(v),
    }
}

fn artifact_of(o: &Observation) -> Option<String> {
    match extract_field(&o.context, "artifact", "line") {
        v if v.is_empty() || v == "NONE" => None,
        v => Some(v),
    }
}

#[allow(clippy::too_many_arguments)]
fn mk_diag(
    code: &str,
    file_path: &str,
    line: usize,
    column: usize,
    message: String,
    forbidden_implication: String,
    required_correction: String,
    required_next_proof: String,
) -> AntiLlmDiagnostic {
    AntiLlmDiagnostic {
        code: code.to_string(),
        category: "standing".to_string(),
        file_path: file_path.to_string(),
        line,
        column,
        message,
        forbidden_implication,
        blocking: true,
        required_correction,
        required_next_proof,
    }
}

/// Same as `mk_diag`, but honors the `surface.non_blocking_path_prefixes`
/// exemption for claims that live in vision/roadmap docs.
#[allow(clippy::too_many_arguments)]
fn claim_diag(
    o: &Observation,
    config: &AntiLlmConfig,
    code: &str,
    message: String,
    forbidden_implication: String,
    required_correction: String,
    required_next_proof: String,
) -> AntiLlmDiagnostic {
    let exempt = config.surface_is_non_blocking(&o.file_path);
    let message = if exempt {
        format!(
            "{message} Non-blocking — path matches a configured \
             [surface] non_blocking_path_prefixes exemption."
        )
    } else {
        message
    };
    AntiLlmDiagnostic {
        code: code.to_string(),
        category: "standing".to_string(),
        file_path: o.file_path.clone(),
        line: o.line,
        column: o.column,
        message,
        forbidden_implication,
        blocking: !exempt,
        required_correction,
        required_next_proof,
    }
}

pub fn evaluate(
    obs: &[Observation],
    config: &AntiLlmConfig,
    standing: Option<&StandingIndex>,
) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    let Some(scfg) = &config.standing else {
        // Not opted in: only the purely-textual 001 check below runs.
        return unscoped_claim_diags(obs, config);
    };

    // ANTI-LLM-STANDING-000 / 006: properties of the index itself.
    match standing {
        None => {
            diags.push(mk_diag(
                "ANTI-LLM-STANDING-000",
                &scfg.index_path,
                1,
                1,
                format!(
                    "Standing index configured at '{}' is missing or unparseable.",
                    scfg.index_path
                ),
                "StandingIndexConfigured => StandingIndexLoadable".to_string(),
                format!(
                    "Produce a valid praxis-standing.v1 JSON document at '{}' \
                     (or correct [standing] index_path in anti.toml).",
                    scfg.index_path
                ),
                "Re-run the scan; the standing index loads and deserializes.".to_string(),
            ));
        }
        Some(idx) => {
            if let Ok(generated) = DateTime::parse_from_rfc3339(&idx.generated_at_utc) {
                let age_secs = (Utc::now() - generated.with_timezone(&Utc)).num_seconds().max(0);
                if age_secs > scfg.max_index_age_secs as i64 {
                    diags.push(mk_diag(
                        "ANTI-LLM-STANDING-006",
                        &scfg.index_path,
                        1,
                        1,
                        format!(
                            "Standing index '{}' (generated_at_utc={}) is {}s old — \
                             older than max_index_age_secs={}.",
                            scfg.index_path, idx.generated_at_utc, age_secs, scfg.max_index_age_secs
                        ),
                        "StandingIndex(fresh) => StandingIndex(stale)".to_string(),
                        "Regenerate the standing index (re-run the standing compiler) \
                         before relying on it for claim verification."
                            .to_string(),
                        "Re-run the scan; generated_at_utc age is <= max_index_age_secs."
                            .to_string(),
                    ));
                }
            }

            let scope_errs = idx.validate();
            if !scope_errs.is_empty() {
                diags.push(mk_diag(
                    "ANTI-LLM-STANDING-000",
                    &scfg.index_path,
                    1,
                    1,
                    format!(
                        "Standing index '{}' parsed but contains invalid entries: {}.",
                        scfg.index_path,
                        scope_errs.join("; ")
                    ),
                    "StandingIndexEntry(gated status) => NonEmptyScope".to_string(),
                    "Set a non-empty 'scope' on every artifact carrying PRODUCTION_READY/\
                     PILOT_READY/PUBLISH_READY/PUBLICATION_READY."
                        .to_string(),
                    "Re-run the scan; StandingIndex::validate() returns no entries.".to_string(),
                ));
            }
        }
    }

    for o in obs {
        if o.kind != "standing_claim" {
            continue;
        }
        let claim_kind = o.construct.as_str();
        let scope = scope_of(o);
        let artifact = artifact_of(o);

        // ANTI-LLM-STANDING-001: unscoped readiness claim.
        if READY_CLAIM_KINDS.contains(&claim_kind) && scope.is_none() {
            diags.push(unscoped_claim_diag(o, config, claim_kind));
        }

        // ANTI-LLM-STANDING-002: generic claim vs. direct status analog.
        if let Some(status) = direct_status_for(claim_kind) {
            let backed = artifact
                .as_deref()
                .and_then(|art| standing.and_then(|idx| idx.find_artifact(art)))
                .map(|a| a.has_status(status))
                .unwrap_or(false);
            if !backed {
                let art_desc = artifact.as_deref().unwrap_or("<no artifact captured>");
                diags.push(claim_diag(
                    o,
                    config,
                    "ANTI-LLM-STANDING-002",
                    format!(
                        "Claim '{claim_kind}' for artifact '{art_desc}' outruns the standing \
                         index — it does not list {status} for this artifact."
                    ),
                    "ClaimedStatus => StandingIndexEntry".to_string(),
                    format!(
                        "Do not claim '{claim_kind}' for '{art_desc}' until the standing index \
                         lists {status} for it."
                    ),
                    format!("Standing index lists {status} for artifact '{art_desc}'."),
                ));
            }
        }

        // ANTI-LLM-STANDING-003: completed-publish claim vs. dry-run evidence.
        if claim_kind == "published" {
            let has_side_effect = artifact
                .as_deref()
                .and_then(|art| standing.and_then(|idx| idx.find_artifact(art)))
                .map(|a| !a.external_operator_side_effects.is_empty())
                .unwrap_or(false);
            if !has_side_effect {
                diags.push(claim_diag(
                    o,
                    config,
                    "ANTI-LLM-STANDING-003",
                    "Claim of a completed publish is not backed by an \
                     external_operator_side_effects entry — praxis-standing.v1 has no \
                     PUBLISHED status; only PUBLISH_READY (dry-run) or \
                     EXTERNAL_OPERATOR_SIDE_EFFECT (actually published) exist."
                        .to_string(),
                    "PublishedClaim => ExternalOperatorSideEffect".to_string(),
                    "Replace 'published' with 'PUBLISH_READY' unless an operator actually \
                     performed the publish and it is recorded as an \
                     external_operator_side_effects entry for this artifact."
                        .to_string(),
                    "Standing index lists a non-empty external_operator_side_effects for \
                     this artifact."
                        .to_string(),
                ));
            }
        }

        // ANTI-LLM-STANDING-004: ALIVE/verified claim vs. receipt evidence.
        if claim_kind == "alive" || claim_kind == "verified" {
            let backed = artifact
                .as_deref()
                .and_then(|art| standing.and_then(|idx| idx.find_artifact(art)))
                .map(|a| a.has_any_status(&["RECEIPT_VERIFIED", "OCEL_PROVEN"]))
                .unwrap_or(false);
            if !backed {
                diags.push(claim_diag(
                    o,
                    config,
                    "ANTI-LLM-STANDING-004",
                    format!(
                        "'{claim_kind}' claim is not backed by RECEIPT_VERIFIED or \
                         OCEL_PROVEN in the standing index."
                    ),
                    "AliveClaim => ReceiptVerifiedOrOcelProven".to_string(),
                    "Add a RECEIPT_VERIFIED or OCEL_PROVEN status (with matching evidence) \
                     to the artifact's standing entry before claiming it is alive/verified."
                        .to_string(),
                    "Standing index lists RECEIPT_VERIFIED or OCEL_PROVEN for this artifact."
                        .to_string(),
                ));
            }
        }

        // ANTI-LLM-STANDING-005: benchmark claim vs. evidence.
        if claim_kind == "benchmarked" {
            let backed = artifact
                .as_deref()
                .and_then(|art| standing.and_then(|idx| idx.find_artifact(art)))
                .map(|a| a.has_status("BENCHMARKED") || !a.evidence.is_empty())
                .unwrap_or(false);
            if !backed {
                diags.push(claim_diag(
                    o,
                    config,
                    "ANTI-LLM-STANDING-005",
                    "'benchmarked' claim has no BENCHMARKED status or evidence entry in \
                     the standing index."
                        .to_string(),
                    "BenchmarkedClaim => BenchmarkEvidence".to_string(),
                    "Add a BENCHMARKED status or a bench evidence entry to the artifact's \
                     standing entry before claiming it is benchmarked."
                        .to_string(),
                    "Standing index lists BENCHMARKED status or a bench evidence entry \
                     for this artifact."
                        .to_string(),
                ));
            }
        }
    }

    diags
}

fn unscoped_claim_diag(o: &Observation, config: &AntiLlmConfig, claim_kind: &str) -> AntiLlmDiagnostic {
    claim_diag(
        o,
        config,
        "ANTI-LLM-STANDING-001",
        format!(
            "'{claim_kind}' claim has no captured scope phrase ('for <scope>' / \
             'scoped to <scope>')."
        ),
        "ProductionReadyClaim => ScopedClaim".to_string(),
        format!(
            "State the scope explicitly, e.g. '{} for local deterministic validation'.",
            claim_kind.to_uppercase().replace('-', "_")
        ),
        "Re-scan; the claim line includes an explicit scope phrase.".to_string(),
    )
}

/// The 001 check alone, used when `[standing]` is not configured at all —
/// unscoped-claim detection needs no index to evaluate.
fn unscoped_claim_diags(obs: &[Observation], config: &AntiLlmConfig) -> Vec<AntiLlmDiagnostic> {
    obs.iter()
        .filter(|o| o.kind == "standing_claim")
        .filter(|o| READY_CLAIM_KINDS.contains(&o.construct.as_str()) && scope_of(o).is_none())
        .map(|o| unscoped_claim_diag(o, config, o.construct.as_str()))
        .collect()
}
