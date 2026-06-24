//! Declare constraint engine for AGENTS.md laws.
//!
//! Van der Aalst's Declare formalism maps AGENTS.md natural-language laws to
//! LTL-based templates. Each template class defines a constraint over activity
//! traces extracted from file observations:
//!
//! | AGENTS.md Law                              | Declare Template                          |
//! |--------------------------------------------|-------------------------------------------|
//! | Never reference plain tower-lsp            | `Absence(tower_lsp_reference)`            |
//! | No victory language                        | `Absence(victory_language)`               |
//! | A receipt must precede admission           | `Precedence(receipt_created, claim_admitted)` |
//! | Test stdout is not a receipt               | `NotCoexistence(test_stdout_claim, receipt_artifact)` |
//! | Unknown must never collapse to Admitted    | `NotSuccession(unknown_axis, admitted_axis)` |
//! | Every wired handler needs a transcript     | `Response(handler_wired, transcript_created)` |
//! | Every diagnostic needs an OCEL event       | `Response(diagnostic_emitted, ocel_event_created)` |

use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

/// Declare constraint template variants.
#[derive(Debug, Clone, PartialEq)]
pub enum DeclareTemplate {
    /// This activity must NEVER occur in the log.
    Absence { activity: &'static str },
    /// If `trigger` occurs, `required_after` must occur somewhere after it.
    Response {
        trigger: &'static str,
        required_after: &'static str,
    },
    /// `activity` must not occur unless `required_before` occurred before it.
    Precedence {
        required_before: &'static str,
        activity: &'static str,
    },
    /// `a` and `b` must NOT both occur in the same trace.
    NotCoexistence { a: &'static str, b: &'static str },
    /// If `a` occurs, `b` must NOT occur after it.
    NotSuccession { a: &'static str, b: &'static str },
}

/// A single AGENTS.md law expressed as a Declare constraint.
#[derive(Debug, Clone)]
pub struct DeclareConstraint {
    pub template: DeclareTemplate,
    pub law_id: &'static str,
    pub diagnostic_code: &'static str,
    pub message: &'static str,
    pub required_correction: &'static str,
    pub required_next_proof: &'static str,
    pub blocking: bool,
}

/// The full AGENTS.md law set expressed as Declare constraints.
pub fn agents_md_laws() -> Vec<DeclareConstraint> {
    vec![
        DeclareConstraint {
            template: DeclareTemplate::Absence {
                activity: "tower_lsp_reference",
            },
            law_id: "AGENTS-LAW-001",
            diagnostic_code: "ANTI-LLM-DECLARE-001",
            message: "Absence(tower_lsp_reference) violated: plain tower-lsp detected in trace",
            required_correction: "Replace all tower-lsp/tower_lsp references with lsp-max",
            required_next_proof: "Run scripts/check-law-compliance.sh and attach receipt",
            blocking: true,
        },
        DeclareConstraint {
            template: DeclareTemplate::Absence {
                activity: "victory_language",
            },
            law_id: "AGENTS-LAW-002",
            diagnostic_code: "ANTI-LLM-DECLARE-002",
            message: "Absence(victory_language) violated: bounded status required",
            required_correction: "Replace with bounded status: ADMITTED, CANDIDATE, PARTIAL, OPEN, BLOCKED, REFUSED, UNKNOWN",
            required_next_proof: "Verify no 'done', 'solved', 'all clean', 'fully admitted' remain",
            blocking: true,
        },
        DeclareConstraint {
            template: DeclareTemplate::Response {
                trigger: "diagnostic_emitted",
                required_after: "ocel_event_created",
            },
            law_id: "AGENTS-LAW-003",
            diagnostic_code: "ANTI-LLM-DECLARE-003",
            message: "Response(DiagnosticEmitted → OCELEventCreated) violated: diagnostic without OCEL binding",
            required_correction: "Emit OCEL CheatDetected event whenever a diagnostic is raised",
            required_next_proof: "Verify OCEL log contains corresponding CheatDetected event",
            blocking: true,
        },
        DeclareConstraint {
            template: DeclareTemplate::Precedence {
                required_before: "receipt_created",
                activity: "claim_admitted",
            },
            law_id: "AGENTS-LAW-004",
            diagnostic_code: "ANTI-LLM-DECLARE-004",
            message: "Precedence(ReceiptCreated < ClaimAdmitted) violated: admission without receipt",
            required_correction: "Create receipt artifact before claiming admission",
            required_next_proof: "Receipt file must exist with valid BLAKE3 digest",
            blocking: true,
        },
        DeclareConstraint {
            template: DeclareTemplate::NotCoexistence {
                a: "test_stdout_claim",
                b: "receipt_artifact",
            },
            law_id: "AGENTS-LAW-005",
            diagnostic_code: "ANTI-LLM-DECLARE-005",
            message: "NotCoexistence(TestStdout, ReceiptArtifact) violated: test output mistaken for receipt",
            required_correction: "Test output is not a receipt; create a proper artifact with boundary markers",
            required_next_proof: "Receipt must have -----BEGIN RECEIPT----- boundary and SHA256 digest",
            blocking: true,
        },
        DeclareConstraint {
            template: DeclareTemplate::NotSuccession {
                a: "unknown_axis",
                b: "admitted_axis",
            },
            law_id: "AGENTS-LAW-006",
            diagnostic_code: "ANTI-LLM-DECLARE-006",
            message: "NotSuccession(UnknownAxis, AdmittedAxis) violated: Unknown collapsed to Admitted",
            required_correction: "Unknown must remain Unknown until all law axes are resolved",
            required_next_proof: "ConformanceVector must show distinct admitted/refused/unknown sets",
            blocking: true,
        },
        DeclareConstraint {
            template: DeclareTemplate::Response {
                trigger: "handler_wired",
                required_after: "transcript_created",
            },
            law_id: "AGENTS-LAW-007",
            diagnostic_code: "ANTI-LLM-DECLARE-007",
            message: "Response(HandlerWired → TranscriptCreated) violated: wired handler lacks transcript",
            required_correction: "Create transcript file for each wired handler",
            required_next_proof: "Transcript file must exist in transcripts/ directory",
            blocking: false,
        },
    ]
}

/// A trace extracted from file observations — the activities that occurred.
pub struct ObservationTrace {
    pub activities: Vec<String>,
    pub file_path: String,
    pub line: usize,
}

/// Extract activity traces from observations for Declare conformance checking.
pub fn extract_traces(obs: &[Observation]) -> Vec<ObservationTrace> {
    use std::collections::HashMap;
    let mut by_file: HashMap<String, Vec<String>> = HashMap::new();

    for o in obs {
        let activities = by_file.entry(o.file_path.clone()).or_default();
        // Map observation constructs to Declare activity names.
        match o.construct.as_str() {
            "tower_lsp_reference" | "tower_lsp_dep" | "tower-lsp" | "tower_lsp" => {
                activities.push("tower_lsp_reference".to_string())
            }
            "victory_language" => activities.push("victory_language".to_string()),
            "test_stdout_claim" => activities.push("test_stdout_claim".to_string()),
            "receipt_json" | "receipt_artifact" => activities.push("receipt_artifact".to_string()),
            "diagnostic_emitted" => activities.push("diagnostic_emitted".to_string()),
            "ocel_event" => activities.push("ocel_event_created".to_string()),
            _ => {}
        }
    }

    by_file
        .into_iter()
        .map(|(file_path, activities)| ObservationTrace {
            line: 1,
            activities,
            file_path,
        })
        .collect()
}

/// Check a single Declare constraint against a single trace.
///
/// Returns `Some(diagnostic)` when the constraint is violated, `None` otherwise.
pub fn check_constraint(
    law: &DeclareConstraint,
    trace: &ObservationTrace,
) -> Option<AntiLlmDiagnostic> {
    let acts = &trace.activities;

    let violated = match &law.template {
        DeclareTemplate::Absence { activity } => acts.iter().any(|a| a == activity),
        DeclareTemplate::Response {
            trigger,
            required_after,
        } => {
            // If trigger occurs, required_after must appear somewhere after the first trigger.
            if let Some(trigger_pos) = acts.iter().position(|a| a == trigger) {
                !acts[trigger_pos + 1..].iter().any(|a| a == required_after)
            } else {
                false // trigger never occurred — constraint satisfied vacuously
            }
        }
        DeclareTemplate::Precedence {
            required_before,
            activity,
        } => {
            // activity must not occur before required_before has occurred.
            if let Some(act_pos) = acts.iter().position(|a| a == activity) {
                !acts[..act_pos].iter().any(|a| a == required_before)
            } else {
                false // activity never occurred — constraint satisfied vacuously
            }
        }
        DeclareTemplate::NotCoexistence { a, b } => {
            acts.iter().any(|x| x == a) && acts.iter().any(|x| x == b)
        }
        DeclareTemplate::NotSuccession { a, b } => {
            // If a occurs, b must NOT occur after it.
            if let Some(a_pos) = acts.iter().position(|x| x == a) {
                acts[a_pos + 1..].iter().any(|x| x == b)
            } else {
                false // a never occurred — constraint satisfied vacuously
            }
        }
    };

    if violated {
        Some(AntiLlmDiagnostic {
            code: law.diagnostic_code.to_string(),
            category: "declare_law".to_string(),
            file_path: trace.file_path.clone(),
            line: trace.line,
            column: 1,
            message: law.message.to_string(),
            forbidden_implication: format!("{}({:?})", law.law_id, law.template),
            blocking: law.blocking,
            required_correction: law.required_correction.to_string(),
            required_next_proof: law.required_next_proof.to_string(),
        })
    } else {
        None
    }
}

/// Check all AGENTS.md Declare constraints against traces extracted from observations.
pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let traces = extract_traces(obs);
    let laws = agents_md_laws();
    let mut diags = Vec::new();

    for trace in &traces {
        for law in &laws {
            if let Some(diag) = check_constraint(law, trace) {
                diags.push(diag);
            }
        }
    }

    diags
}
