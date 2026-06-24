use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

/// Evaluate GGEN-* observations into diagnostics.
///
/// Codes emitted:
///   GGEN-TPL-001   — template variable not produced by paired SPARQL SELECT
///   GGEN-YIELD-001 — output_file targets pack root (no directory component)
///   GGEN-YIELD-002 — output would land in second-class path (also caught by SRC-001)
///   GGEN-YIELD-004 — competing authority: two manifests claim the same output_file
///   GGEN-YIELD-005 — remote ontology fetch in replay path
///   GGEN-SRC-001   — output_file path contains generated/output/gen segment
///   GGEN-SRC-002   — source file contains DO NOT EDIT / auto-generated banner
///   GGEN-SRC-003   — source file comment instructs LLM/human to treat it as lesser source
pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        match o.construct.as_str() {
            // ── TPL-001 ───────────────────────────────────────────────────────
            "ggen_template_var_mismatch" => {
                diags.push(AntiLlmDiagnostic {
                    code: "GGEN-TPL-001".to_string(),
                    category: "ggen_template".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: o.message.clone(),
                    forbidden_implication: "TemplateVar(consumed) ⊄ SPARQLVar(projected) → RenderedOutput.silently_wrong".to_string(),
                    blocking: true,
                    required_correction: "Add the missing ?variable to the paired SPARQL SELECT projection, or remove its reference from the template.".to_string(),
                    required_next_proof: "Run `ggen sync` — it must complete without template rendering errors.".to_string(),
                });
            }

            // ── YIELD-001 ─────────────────────────────────────────────────────
            "ggen_layer_violation" => {
                diags.push(AntiLlmDiagnostic {
                    code: "GGEN-YIELD-001".to_string(),
                    category: "ggen_yield".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("LAYER_VIOLATION: output_file '{}' targets the pack root, not a consumer path", o.context),
                    forbidden_implication: "PackRoot(output_file) → ConsumerRoot(output_file)".to_string(),
                    blocking: true,
                    required_correction: "Set output_file to a path inside a consumer package root (e.g. packages/foo/src/ or crates/bar/src/).".to_string(),
                    required_next_proof: "Run `ggen sync` — rendered file must land in a consumer package.".to_string(),
                });
            }

            // ── YIELD-004 ─────────────────────────────────────────────────────
            "ggen_competing_authority" => {
                diags.push(AntiLlmDiagnostic {
                    code: "GGEN-YIELD-004".to_string(),
                    category: "ggen_yield".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("COMPETING_AUTHORITY: {}", o.context),
                    forbidden_implication: "DualManifest(output_file) → SingleAuthority(output_file)".to_string(),
                    blocking: true,
                    required_correction: "Remove the duplicate output_file declaration from one manifest. Only one ggen.toml may own a given output path.".to_string(),
                    required_next_proof: "Run `ggen sync` — it must complete without competing-authority errors.".to_string(),
                });
            }

            // ── YIELD-005 ─────────────────────────────────────────────────────
            "ggen_remote_fetch" => {
                diags.push(AntiLlmDiagnostic {
                    code: "GGEN-YIELD-005".to_string(),
                    category: "ggen_yield".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("REMOTE_FETCH_PROHIBITED: remote ontology URL '{}' enters replay path", o.context),
                    forbidden_implication: "RemoteFetch(ontology) → ReplayDeterminism".to_string(),
                    blocking: true,
                    required_correction: "Replace the remote ontology URL with a local relative path. Remote fetches are non-deterministic across replay runs.".to_string(),
                    required_next_proof: "Verify ggen.toml contains only local ontology file paths.".to_string(),
                });
            }

            // ── SRC-001 (also covers YIELD-002) ──────────────────────────────
            "ggen_second_class_path" => {
                diags.push(AntiLlmDiagnostic {
                    code: "GGEN-SRC-001".to_string(),
                    category: "ggen_source".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("SECOND_CLASS_PATH: output_file '{}' contains a generated/output/gen segment", o.context),
                    forbidden_implication: "SecondClassPath(output_file) → FirstClassSource(output_file)".to_string(),
                    blocking: true,
                    required_correction: "Move output_file to a first-class source path — not under generated/, output/, or gen/.".to_string(),
                    required_next_proof: "Run `ggen sync` — rendered source must land at a first-class path.".to_string(),
                });
            }

            // ── SRC-002 ───────────────────────────────────────────────────────
            "ggen_do_not_edit_banner" => {
                diags.push(AntiLlmDiagnostic {
                    code: "GGEN-SRC-002".to_string(),
                    category: "ggen_source".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("DO_NOT_EDIT_CASTE: '{}' — ggen-provided source is first-class; this banner is forbidden", o.context.trim()),
                    forbidden_implication: "DONOTEDITBanner(source) → FirstClassSource(source)".to_string(),
                    blocking: true,
                    required_correction: "Remove the DO NOT EDIT / auto-generated banner. Inspect and repair the template if it emits the banner.".to_string(),
                    required_next_proof: "Verify no auto-generated banners remain in the file after re-running ggen sync.".to_string(),
                });
            }

            // ── SRC-003 ───────────────────────────────────────────────────────
            "ggen_lesser_source_comment" => {
                diags.push(AntiLlmDiagnostic {
                    code: "GGEN-SRC-003".to_string(),
                    category: "ggen_source".to_string(),
                    file_path: o.file_path.clone(),
                    line: o.line,
                    column: o.column,
                    message: format!("SOURCE_CASTE_COMMENT: comment instructs reader to treat file as lesser source: '{}'", o.context.trim()),
                    forbidden_implication: "LesserSourceComment(source) → FirstClassSource(source)".to_string(),
                    blocking: true,
                    required_correction: "Remove the comment. All ggen-rendered source is first-class and may be inspected, reasoned over, and repaired.".to_string(),
                    required_next_proof: "Verify the comment is removed. If it was emitted by the template, fix the .tera seed.".to_string(),
                });
            }

            _ => {}
        }
    }

    diags
}
