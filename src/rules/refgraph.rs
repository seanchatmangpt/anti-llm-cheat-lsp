//! Rule for cross-product #4 — transitive failset over the reference graph.
//!
//! Emits `ANTI-LLM-REFGRAPH-001` for each site that the bounded reference
//! closure (`parsers::refgraph`) proved to transitively depend on an
//! unwitnessed symbol. The diagnostic is a CANDIDATE-level signal: it is raised
//! only on explicit reverse-reachability within the stated depth bound, so a
//! site with no chain to a seed is never flagged.

use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        if o.kind != "failset_member" {
            continue;
        }
        diags.push(AntiLlmDiagnostic {
            code: "ANTI-LLM-REFGRAPH-001".to_string(),
            category: "refgraph".to_string(),
            file_path: o.file_path.clone(),
            line: o.line,
            column: o.column,
            message: o.message.clone(),
            forbidden_implication: "DependentSite => Witnessed".to_string(),
            blocking: true,
            required_correction: "This symbol is reverse-reachable, within the bounded reference closure, from a symbol declared unwitnessed (`// @unwitnessed:`). A dependent of an unwitnessed symbol inherits its UNKNOWN status; it must not be treated as ADMITTED. Either discharge the seed symbol's witness or sever the reference dependency.".to_string(),
            required_next_proof: "Seed symbol carries an admitted witness (receipt with path/digest/boundary/negative-control), OR the reference edge to the seed is removed and re-scan shows the site absent from the failset.".to_string(),
        });
    }

    diags
}
