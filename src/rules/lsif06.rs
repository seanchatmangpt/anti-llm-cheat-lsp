//! LSIF 0.6 combinatorial surface enumeration.
//!
//! The LSIF 0.6 element graph is modelled in the `lsp-max-lsif` crate. The
//! anti-llm-cheat-lsp example carries no LSIF transcripts or receipts, so every
//! row's example-coverage axis is honestly `OPEN`. Elements that the
//! `lsp-max-lsif` crate models are `CANDIDATE` for coverage (the type exists)
//! but never rise above `OPEN` here until a transcript + receipt is produced.
//! Elements absent from the crate are `UNKNOWN` — neither modelled nor refused.

/// Whether an LSIF element is a vertex or an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    Vertex,
    Edge,
}

impl ElementKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ElementKind::Vertex => "vertex",
            ElementKind::Edge => "edge",
        }
    }
}

/// One LSIF 0.6 element and the static facts about it.
#[derive(Debug, Clone)]
pub struct LsifElement {
    pub name: &'static str,
    pub kind: ElementKind,
    /// Modelled as a type/variant in the `lsp-max-lsif` crate.
    pub modeled_in_crate: bool,
    /// Modelled only via codegen (`auto_generated`), not the hand-authored enum.
    pub codegen_only: bool,
}

/// The full LSIF 0.6 vertex + edge surface.
pub fn full_surface() -> Vec<LsifElement> {
    use ElementKind::*;
    let rows: &[(&'static str, ElementKind, bool, bool)] = &[
        // ── Vertices ──────────────────────────────────────────────────────
        ("metaData", Vertex, true, false),
        ("source", Vertex, true, false),
        ("capabilities", Vertex, true, false),
        ("project", Vertex, true, false),
        ("document", Vertex, true, false),
        ("range", Vertex, true, false),
        ("resultSet", Vertex, true, false),
        ("moniker", Vertex, true, false),
        ("packageInformation", Vertex, true, false),
        ("hoverResult", Vertex, true, false),
        ("definitionResult", Vertex, true, false),
        ("declarationResult", Vertex, true, false),
        ("typeDefinitionResult", Vertex, true, false),
        ("referenceResult", Vertex, true, false),
        ("implementationResult", Vertex, true, false),
        ("documentSymbolResult", Vertex, true, false),
        ("foldingRangeResult", Vertex, true, false),
        ("documentLinkResult", Vertex, true, false),
        ("diagnosticResult", Vertex, true, false),
        ("$event", Vertex, true, false),
        // ── Edges ─────────────────────────────────────────────────────────
        ("contains", Edge, true, false),
        ("item", Edge, true, false),
        ("next", Edge, true, false),
        ("moniker", Edge, true, false),
        ("packageInformation", Edge, true, false),
        ("nextMoniker", Edge, true, false),
        ("attach", Edge, true, false),
        ("belongsTo", Edge, true, false),
        ("textDocument/definition", Edge, true, false),
        ("textDocument/declaration", Edge, true, false),
        ("textDocument/typeDefinition", Edge, true, false),
        ("textDocument/references", Edge, true, false),
        ("textDocument/implementation", Edge, true, false),
        ("textDocument/hover", Edge, true, false),
        ("textDocument/documentSymbol", Edge, true, false),
        ("textDocument/foldingRange", Edge, true, false),
        ("textDocument/documentLink", Edge, true, false),
        ("textDocument/diagnostic", Edge, true, false),
    ];

    rows.iter()
        .map(|&(name, kind, modeled, codegen)| LsifElement {
            name,
            kind,
            modeled_in_crate: modeled,
            codegen_only: codegen,
        })
        .collect()
}

/// One evidence-derived LSIF coverage row.
#[derive(Debug, Clone)]
pub struct LsifCoverageRow {
    pub name: String,
    pub kind: String,
    pub modeled_in_crate: bool,
    pub example_status: String,
}

/// Derive the honest example-coverage status for an LSIF element. The example
/// carries no LSIF transcripts/receipts, so modelled elements are `OPEN`
/// (CANDIDATE for coverage, but not yet evidenced) and unmodelled elements are
/// `UNKNOWN`. `codegen_only` elements are `PARTIAL` — wired only via codegen.
fn derive_example_status(modeled: bool, codegen_only: bool) -> &'static str {
    if !modeled {
        "UNKNOWN"
    } else if codegen_only {
        "PARTIAL"
    } else {
        "OPEN"
    }
}

pub fn compute_coverage() -> Vec<LsifCoverageRow> {
    full_surface()
        .into_iter()
        .map(|e| LsifCoverageRow {
            name: e.name.to_string(),
            kind: e.kind.as_str().to_string(),
            modeled_in_crate: e.modeled_in_crate,
            example_status: derive_example_status(e.modeled_in_crate, e.codegen_only).to_string(),
        })
        .collect()
}

/// Bounded LSIF coverage summary.
#[derive(Debug, Clone, Default)]
pub struct LsifSummary {
    pub total: usize,
    pub vertices: usize,
    pub edges: usize,
    pub modeled_in_crate: usize,
    pub covered_by_example: usize,
}

pub fn lsif_summary(rows: &[LsifCoverageRow]) -> LsifSummary {
    let mut s = LsifSummary {
        total: rows.len(),
        ..Default::default()
    };
    for r in rows {
        if r.kind == "vertex" {
            s.vertices += 1;
        } else {
            s.edges += 1;
        }
        if r.modeled_in_crate {
            s.modeled_in_crate += 1;
        }
        // No LSIF transcript/receipt exists in the example, so coverage stays 0.
        if r.example_status == "ADMITTED" {
            s.covered_by_example += 1;
        }
    }
    s
}
