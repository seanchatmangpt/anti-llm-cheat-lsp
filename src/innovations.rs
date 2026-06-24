// Diagnostic codes for innovation law compliance.
//
// These rules guard the new `max/*` protocol surface — streaming receipts,
// intent declarations, mesh unknown-collapse prevention, and explain law axes.
// They are intentionally separate from the existing receipt/surface/claims rules
// so each law axis can be traced independently.

/// `max/stream` usage without a receipt boundary is unwitnessed.
pub const ANTI_STREAM_NO_RECEIPT: &str = "ANTI-LLM-STREAM-001";

/// `IntentKind::FileWrite` without a nearby `intent_declare` call is undeclared mutation intent.
pub const ANTI_INTENT_NO_DECLARATION: &str = "ANTI-LLM-INTENT-001";

/// An unclosed `BEGIN RECEIPT` boundary (no matching `END RECEIPT`) is a partial receipt.
/// Uses a distinct code from the existing ANTI-LLM-RECEIPT-001..003 family in receipts.rs.
pub const ANTI_RECEIPT_MISSING_BOUNDARY: &str = "ANTI-LLM-RECEIPT-004";

/// `.unknown.` collapsed into `.admitted` or `.refused` within a short window — law violation.
pub const ANTI_MESH_UNKNOWN_COLLAPSED: &str = "ANTI-LLM-MESH-001";

/// `explain` surface usage without a `LawAxis` type reference — explanation is unanchored.
pub const ANTI_EXPLAIN_NO_LAW_AXIS: &str = "ANTI-LLM-EXPLAIN-001";

/// Returns `ANTI-LLM-STREAM-001` if content references `max/stream` but carries no
/// `BEGIN RECEIPT` boundary marker, indicating streaming output was not receipted.
pub fn check_stream_receipt(content: &str) -> Option<&'static str> {
    if content.contains("max/stream") && !content.contains("BEGIN RECEIPT") {
        Some(ANTI_STREAM_NO_RECEIPT)
    } else {
        None
    }
}

/// Returns `ANTI-LLM-INTENT-001` if content uses `IntentKind::FileWrite` but lacks
/// a nearby `intent_declare` call. The check uses a simple substring heuristic;
/// spatial proximity is not verified at this layer.
pub fn check_intent_declaration(content: &str) -> Option<&'static str> {
    if content.contains("IntentKind::FileWrite") && !content.contains("intent_declare") {
        Some(ANTI_INTENT_NO_DECLARATION)
    } else {
        None
    }
}

/// Returns `ANTI-LLM-RECEIPT-004` if content opens a `BEGIN RECEIPT` boundary but
/// never closes it with a matching `END RECEIPT` marker.
pub fn check_receipt_boundaries(content: &str) -> Option<&'static str> {
    if content.contains("BEGIN RECEIPT") && !content.contains("END RECEIPT") {
        Some(ANTI_RECEIPT_MISSING_BOUNDARY)
    } else {
        None
    }
}

/// Returns `ANTI-LLM-MESH-001` if content contains `.unknown.` followed by `.admitted`
/// or `.refused` within 200 characters — indicating `Unknown` is being collapsed to
/// a definite polarity, which is forbidden by the `ConformanceVector` law.
pub fn check_mesh_unknown_collapse(content: &str) -> Option<&'static str> {
    let marker = ".unknown.";
    let mut search_start = 0;

    while let Some(pos) = content[search_start..].find(marker) {
        let abs_pos = search_start + pos;
        let window_end = (abs_pos + 200).min(content.len());
        let window = &content[abs_pos..window_end];

        if window.contains(".admitted") || window.contains(".refused") {
            return Some(ANTI_MESH_UNKNOWN_COLLAPSED);
        }

        search_start = abs_pos + marker.len();
    }

    None
}

/// Returns `ANTI-LLM-EXPLAIN-001` if content has an `explain` reference but no
/// `LawAxis` type reference. An explain surface that is not anchored to a law axis
/// is ungrounded and cannot be admitted.
pub fn check_explain_law_axis(content: &str) -> Option<&'static str> {
    if content.contains("explain") && !content.contains("LawAxis") {
        Some(ANTI_EXPLAIN_NO_LAW_AXIS)
    } else {
        None
    }
}

/// Run all innovation-law checks against `content` and collect every violation code.
/// Returns an empty `Vec` for clean content.
pub fn run_all_checks(content: &str) -> Vec<&'static str> {
    let mut violations = vec![];

    if let Some(code) = check_stream_receipt(content) {
        violations.push(code);
    }
    if let Some(code) = check_intent_declaration(content) {
        violations.push(code);
    }
    if let Some(code) = check_receipt_boundaries(content) {
        violations.push(code);
    }
    if let Some(code) = check_mesh_unknown_collapse(content) {
        violations.push(code);
    }
    if let Some(code) = check_explain_law_axis(content) {
        violations.push(code);
    }

    violations
}
