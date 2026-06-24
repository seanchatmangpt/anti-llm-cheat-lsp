//! Cross-product #4 — transitive cheat-detection through the reference graph.
//!
//! This module is the *additive* reference-graph factor. It does not replace the
//! single-file passes or the existing cross-file passes
//! (`contract::detect_contract_schism`, `ggen_toml::detect_competing_authority`);
//! it joins their observations into one symbol index and propagates a failset.
//!
//! ## Symbol identity
//!
//! Symbols are keyed by their **moniker content identity** — the bare terminal
//! segment of a Rust path (`a::b::foo` ⇒ `foo`). This matches the Phase-A LSIF
//! moniker key produced by `lsp_max_lsif::lsif_indexer::emit_moniker`, where the
//! moniker `identifier` is the content-addressed symbol name rather than a
//! file-local range id. Reference edges arrive as `fn_reference` observations
//! (`construct` = callee key, `context` = caller key) emitted by the Rust
//! tree-sitter parser; these are the analogue of LSIF `references` /
//! moniker `import`→`export` `attach` linkage.
//!
//! ## Bound (stated explicitly — NOT general-graph soundness)
//!
//! The failset closure is **bounded** on three axes:
//!
//! 1. **Direction**: edges are followed only in the reverse caller→callee
//!    direction (a *referrer* of an unwitnessed symbol). This mirrors following
//!    `import`→`export` `attach` edges back to the exporting symbol.
//! 2. **Depth**: traversal is depth-limited to [`MAX_DEPTH`]. A site farther than
//!    [`MAX_DEPTH`] reverse-edges from a seed is NOT added to the failset.
//! 3. **Explicitness**: a site enters the failset only if there is an explicit
//!    chain of `fn_reference` edges reaching a seed. There is no implicit,
//!    heuristic, or name-similarity edge. Absence of a chain ⇒ absence from the
//!    failset.
//!
//! The closure is **monotone**: adding observations can only grow the failset,
//! never shrink it. Within this bound the negative control is ADMITTED — a
//! symbol with no explicit reverse-reachable chain to a seed can never be added,
//! so it cannot raise a transitive ANDON. General-graph soundness (arbitrary
//! aliasing, dynamic dispatch, macro-expanded calls, FFI) is **OPEN** and is not
//! claimed here.

use crate::observations::Observation;
use std::collections::{HashMap, HashSet, VecDeque};

/// Extract failset seeds from a Rust source body.
///
/// A symbol is declared unwitnessed by an explicit author marker
/// `// @unwitnessed: <fn_name>` (one per line). This is the ONLY source of
/// failset seeds — there is no heuristic inference of unwitnessed-ness, which is
/// precisely what keeps the negative control sound: a symbol the author never
/// marked can never become a seed.
pub fn extract_unwitnessed_seeds(filepath: &str, content: &str) -> Vec<Observation> {
    let mut out = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        if let Some(rest) = line.split("// @unwitnessed:").nth(1) {
            let name = rest.trim();
            if !name.is_empty() {
                out.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: line_idx + 1,
                    column: 1,
                    kind: "unwitnessed_symbol".to_string(),
                    construct: name.to_string(),
                    context: filepath.to_string(),
                    message: format!("Symbol '{}' declared unwitnessed (failset seed)", name),
                });
            }
        }
    }
    out
}

/// Maximum reverse-edge depth explored from any unwitnessed seed.
///
/// Beyond this depth a dependent site is left OUT of the failset rather than
/// guessed at. This is the depth axis of the stated bound.
pub const MAX_DEPTH: usize = 8;

/// Build the bounded transitive failset and emit one observation per dependent
/// site found within [`MAX_DEPTH`] reverse-reachable edges of an unwitnessed
/// seed symbol.
///
/// Additive: this consumes the already-collected `all_obs` and appends new
/// `failset_member` observations. It never removes or rewrites prior
/// observations.
pub fn detect_transitive_failset(all_obs: &[Observation]) -> Vec<Observation> {
    let mut out = Vec::new();

    // Seeds: explicitly-declared unwitnessed symbol keys.
    let seeds: HashSet<&str> = all_obs
        .iter()
        .filter(|o| o.kind == "unwitnessed_symbol")
        .map(|o| o.construct.as_str())
        .collect();
    if seeds.is_empty() {
        return out;
    }

    // Reverse adjacency: callee_key -> set of caller_keys (referrers).
    // Each `fn_reference` is a forward edge caller(context) -> callee(construct);
    // we index it reversed so a BFS from a seed walks toward its dependents.
    let mut referrers: HashMap<&str, HashSet<&str>> = HashMap::new();
    // First site (file + line) at which a given caller key is defined/observed,
    // so the emitted observation points at a real location.
    let mut caller_site: HashMap<&str, &Observation> = HashMap::new();

    for o in all_obs {
        if o.kind == "fn_reference" {
            let callee = o.construct.as_str();
            let caller = o.context.as_str();
            referrers.entry(callee).or_default().insert(caller);
            caller_site.entry(caller).or_insert(o);
        }
    }

    // Bounded reverse BFS. `visited` is keyed by symbol so the closure is a set
    // (monotone, terminating). Depth is tracked per frontier entry.
    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<(&str, usize)> = VecDeque::new();

    for &seed in &seeds {
        queue.push_back((seed, 0));
        visited.insert(seed);
    }

    while let Some((sym, depth)) = queue.pop_front() {
        if depth >= MAX_DEPTH {
            // Bound reached — do NOT expand further. Dependents beyond this
            // horizon are intentionally left UNKNOWN rather than flagged.
            continue;
        }
        let Some(callers) = referrers.get(sym) else {
            continue;
        };
        for &caller in callers {
            if !visited.insert(caller) {
                continue;
            }
            // A genuine transitive dependent (depth+1 edges from a seed).
            let site = caller_site.get(caller).copied();
            let (file_path, line, column) = match site {
                Some(o) => (o.file_path.clone(), o.line, o.column),
                None => ("unknown".to_string(), 1, 1),
            };
            out.push(Observation {
                file_path,
                start_byte: 0,
                end_byte: 0,
                line,
                column,
                kind: "failset_member".to_string(),
                construct: caller.to_string(),
                context: format!("depth={}", depth + 1),
                message: format!(
                    "Symbol '{}' transitively depends on an unwitnessed symbol \
                     ({} reverse-reference hop(s) within bound {}) — CANDIDATE failset member",
                    caller,
                    depth + 1,
                    MAX_DEPTH
                ),
            });
            queue.push_back((caller, depth + 1));
        }
    }

    out
}
