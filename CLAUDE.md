# anti-llm-cheat-lsp

Diagnostic canary that detects lsp-max law violations using an AhoCorasick multi-pattern engine. Implements `RulePackServer` via the engine-bridge pattern.

## Architecture

```
engine.rs           — AhoCorasick multi-format scanner
observations.rs     — raw observation types
diagnostics.rs      — AntiLlmDiagnostic → LSP Diagnostic conversion
server.rs           — AntiLlmServer: RulePackServer + LanguageServer impl
capabilities.rs     — LSP 3.18 ServerCapabilities (matrix-derived)
config.rs           — centralized vocabulary (victory terms, forbidden patterns)
ast_adapter.rs      — RustAstAdapter wrapping AutoLspAdapter
semantic.rs         — AST-driven SemanticTokens
rules/              — coverage matrices: lsp318, lsif06, lsp318_full_matrix
virtual_docs/       — virtual document modules (render fns per URI scheme)
```

## Key Invariants

- `scan_uri_classified` bridges `engine::scan_directory + evaluate_diagnostics` into `ClassifiedFindings`
- `WorkspaceIndex` is wired — `handle_did_*` calls `upsert/remove` automatically
- Virtual docs are served from `text_document_content` match arms; never from files
- `ValidatedRulePackSet::empty()` — no TOML packs; engine-bridge server
- `LawAxis::Custom(d.category.clone())` — always use `Custom` for diagnostic categories

## Adding a Virtual Document

1. Create `virtual_docs/<name>.rs` with a `pub fn render(...) -> String`
2. Add `pub mod <name>;` to `virtual_docs/mod.rs`
3. Add match arm in `server.rs::text_document_content()`:
   ```rust
   "anti-llm://<name>" => Some(virtual_docs::<name>::render(&diagnostics)),
   ```

No file I/O, no mutations. Virtual docs are computed from live state.

## Testing

```bash
cargo test -p anti-llm-cheat-lsp                    # unit tests
cargo test -p anti-llm-cheat-lsp --test dogfood     # dogfood integration test
```

Dogfood test exercises the server end-to-end against fixture files. New fixtures go in `fixtures/`.

Negative controls live in `fixtures/negative_controls/`. Each must trigger at least one diagnostic.

## Diagnostic Codes

```
ANTI-LLM-TOWER-*     — plain tower-lsp reference
ANTI-LLM-VICTORY-*   — victory language in code/comments/docs
ANTI-LLM-CLAIMS-*    — overclaim in status words
ANTI-LLM-RECEIPT-*   — fake receipt (no boundary markers, no digest)
ANTI-LLM-ROUTE-*     — log-as-route-proof substitution
ANTI-LLM-VERSION-*   — CalVer violation (SemVer detected)
WASM4PM-*            — process-mining law violation (triggers gate ANDON)
GGEN-*               — ggen violation (triggers gate ANDON)
```

## Law Status

- Virtual doc `anti-llm://process-model`: CANDIDATE (renders live DFG + Declare)
- `RulePackServer` bridge: CANDIDATE
- `WorkspaceIndex` wiring: CANDIDATE
- LSP 3.18 coverage: PARTIAL (93/95 methods Wired or Refuses)
- LSIF 0.6 coverage: PARTIAL (all 38 elements modeled; no transcripts yet)
