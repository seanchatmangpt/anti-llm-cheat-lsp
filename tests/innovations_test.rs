// Unit tests for innovation law compliance checks.
// Pure string matching — no async runtime needed.

use anti_llm_cheat_lsp::innovations::{
    check_explain_law_axis, check_intent_declaration, check_mesh_unknown_collapse,
    check_receipt_boundaries, check_stream_receipt, run_all_checks, ANTI_EXPLAIN_NO_LAW_AXIS,
    ANTI_INTENT_NO_DECLARATION, ANTI_MESH_UNKNOWN_COLLAPSED, ANTI_RECEIPT_MISSING_BOUNDARY,
    ANTI_STREAM_NO_RECEIPT,
};

// ── check_receipt_boundaries ──────────────────────────────────────────────────

#[test]
fn receipt_boundaries_catches_unclosed_begin() {
    let content = "-----BEGIN RECEIPT-----\ndigest: abc123\n";
    assert_eq!(
        check_receipt_boundaries(content),
        Some(ANTI_RECEIPT_MISSING_BOUNDARY)
    );
}

#[test]
fn receipt_boundaries_clean_when_both_markers_present() {
    let content = "-----BEGIN RECEIPT-----\ndigest: abc123\n-----END RECEIPT-----\n";
    assert_eq!(check_receipt_boundaries(content), None);
}

#[test]
fn receipt_boundaries_clean_when_no_markers() {
    let content = "fn main() { /* no receipt markers here */ }";
    assert_eq!(check_receipt_boundaries(content), None);
}

// ── check_mesh_unknown_collapse ───────────────────────────────────────────────

#[test]
fn mesh_unknown_collapse_catches_admitted_within_window() {
    // .unknown. followed by .admitted within 200 chars is a law violation.
    let content = "vector.unknown.iter().map(|x| x.admitted).collect()";
    assert_eq!(
        check_mesh_unknown_collapse(content),
        Some(ANTI_MESH_UNKNOWN_COLLAPSED)
    );
}

#[test]
fn mesh_unknown_collapse_catches_refused_within_window() {
    let content = "vector.unknown.iter().map(|x| x.refused).collect()";
    assert_eq!(
        check_mesh_unknown_collapse(content),
        Some(ANTI_MESH_UNKNOWN_COLLAPSED)
    );
}

#[test]
fn mesh_unknown_collapse_clean_when_no_collapse() {
    // .unknown. present but no .admitted or .refused follows within 200 chars.
    let content = "// unknown state — tracing in progress";
    assert_eq!(check_mesh_unknown_collapse(content), None);
}

#[test]
fn mesh_unknown_collapse_clean_when_collapse_beyond_window() {
    // .admitted appears after 200 chars — outside the detection window.
    let filler = "x".repeat(201);
    let content = format!(".unknown.{}.admitted", filler);
    assert_eq!(check_mesh_unknown_collapse(&content), None);
}

// ── check_stream_receipt ──────────────────────────────────────────────────────

#[test]
fn stream_receipt_catches_missing_receipt() {
    let content = r#"{"method": "max/stream", "params": {}}"#;
    assert_eq!(check_stream_receipt(content), Some(ANTI_STREAM_NO_RECEIPT));
}

#[test]
fn stream_receipt_clean_when_receipt_present() {
    let content =
        r#"{"method": "max/stream"} -----BEGIN RECEIPT----- digest: abc -----END RECEIPT-----"#;
    assert_eq!(check_stream_receipt(content), None);
}

// ── check_intent_declaration ─────────────────────────────────────────────────

#[test]
fn intent_declaration_catches_missing_declare_call() {
    let content = "let kind = IntentKind::FileWrite; do_write(path);";
    assert_eq!(
        check_intent_declaration(content),
        Some(ANTI_INTENT_NO_DECLARATION)
    );
}

#[test]
fn intent_declaration_clean_when_declare_present() {
    let content = "let kind = IntentKind::FileWrite; intent_declare(kind, path);";
    assert_eq!(check_intent_declaration(content), None);
}

// ── check_explain_law_axis ────────────────────────────────────────────────────

#[test]
fn explain_law_axis_catches_unanchored_explain() {
    let content = "fn explain_result() { /* some explanation */ }";
    assert_eq!(
        check_explain_law_axis(content),
        Some(ANTI_EXPLAIN_NO_LAW_AXIS)
    );
}

#[test]
fn explain_law_axis_clean_when_law_axis_present() {
    let content = "fn explain_result(axis: LawAxis) { axis.explain() }";
    assert_eq!(check_explain_law_axis(content), None);
}

// ── run_all_checks ────────────────────────────────────────────────────────────

#[test]
fn run_all_checks_returns_empty_for_clean_content() {
    let content = "fn compute(x: u32) -> u32 { x + 1 }";
    assert!(
        run_all_checks(content).is_empty(),
        "clean content must produce no violations"
    );
}

#[test]
fn run_all_checks_collects_multiple_violations() {
    // Triggers stream-no-receipt and undeclared mutation intent simultaneously.
    let content = "max/stream called without receipt\nIntentKind::FileWrite but no declare";
    let violations = run_all_checks(content);
    assert!(
        violations.contains(&ANTI_STREAM_NO_RECEIPT),
        "expected ANTI_STREAM_NO_RECEIPT in violations"
    );
    assert!(
        violations.contains(&ANTI_INTENT_NO_DECLARATION),
        "expected ANTI_INTENT_NO_DECLARATION in violations"
    );
}

#[test]
fn run_all_checks_victory_language_not_flagged_by_these_checks() {
    // The word "done" is victory language but is handled by the existing
    // claims rule, not by these innovation checks.
    let violations = run_all_checks("done");
    assert!(
        violations.is_empty(),
        "victory language 'done' must not be flagged by innovation checks"
    );
}
