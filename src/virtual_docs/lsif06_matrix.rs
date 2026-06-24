//! Live LSIF 0.6 combinatorial coverage matrix.
//!
//! Enumerates the full LSIF 0.6 vertex + edge surface and reports how the
//! `lsp-max-lsif` crate models each element and what coverage the example
//! carries. The example has no LSIF transcripts/receipts, so the example axis
//! is honestly `OPEN`/`UNKNOWN`/`PARTIAL` — never inflated to ADMITTED.

use crate::rules::lsif06::{compute_coverage, lsif_summary};

pub fn generate_lsif06_matrix_markdown() -> String {
    let rows = compute_coverage();
    let summary = lsif_summary(&rows);

    let mut out = String::new();
    out.push_str("# LSIF 0.6 Combinatorial Coverage Matrix\n\n");
    out.push_str(
        "Surface enumerated from the LSIF 0.6 element graph. `modeled in crate` reflects the \
`lsp-max-lsif` type surface; the example-coverage status is `OPEN` for modelled elements \
(CANDIDATE for coverage, not yet evidenced by a transcript + receipt), `PARTIAL` for \
codegen-only elements, and `UNKNOWN` for elements the crate does not model.\n\n",
    );

    out.push_str(&format!(
        "Surface — total: {} ({} vertices, {} edges); modelled in crate: {}; covered by example: {}.\n\n",
        summary.total,
        summary.vertices,
        summary.edges,
        summary.modeled_in_crate,
        summary.covered_by_example
    ));

    out.push_str("| Element | Kind | Modelled in crate | Example Coverage Status |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for r in &rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.name,
            r.kind,
            if r.modeled_in_crate { "yes" } else { "no" },
            r.example_status,
        ));
    }

    out
}
