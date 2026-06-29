//! Live LSP 3.18 combinatorial coverage matrix.
//!
//! This is the extractor output that `ANTI-LLM-LSP318-COMB-001` requires: the
//! full method surface with statuses derived from on-disk evidence, not the
//! 15-row delta changelog. It is rendered on demand and never written to disk.

use crate::rules::lsp318_coverage::{compute_coverage, conformance_summary};

pub fn generate_full_matrix_markdown(workspace_root: &str) -> String {
    let rows = compute_coverage(workspace_root);
    let summary = conformance_summary(&rows);

    let mut out = String::new();
    out.push_str("# LSP 3.18 Combinatorial Coverage Matrix (extractor output)\n\n");
    out.push_str(
        "Every row's status is computed from on-disk evidence (transcript file present, \
receipt artifact present), not from a hand-authored claim. A transcript without a wired \
handler is `UNKNOWN`; a wired handler with a transcript reaches \
`SUPPORTED_WITH_TRANSCRIPT` only while the receipt axis stays `OPEN`.\n\n",
    );

    out.push_str(&format!(
        "Conformance axes — total: {}, transcript-admitted: {}, refused: {}, unknown: {}, receipts present: {}.\n\n",
        summary.total, summary.admitted, summary.refused, summary.unknown, summary.receipts_present
    ));

    out.push_str(
        "| Method | Direction | Client Capability Path | Server Capability Path | Transcript | Receipt | Status |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for r in &rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            r.method,
            r.direction,
            if r.client_capability_path.is_empty() { "—" } else { &r.client_capability_path },
            if r.server_capability_path.is_empty() { "—" } else { &r.server_capability_path },
            if r.transcript_present { "present" } else { "NONE" },
            if r.receipt_present { "present" } else { "OPEN" },
            r.status,
        ));
    }

    out
}
