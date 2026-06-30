# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# anti-llm-cheat-lsp

Diagnostic canary that detects lsp-max law violations using an AhoCorasick multi-pattern engine. Implements `RulePackServer` via the engine-bridge pattern.

## Development

### Build & Run

```bash
cargo build                                          # debug build
cargo build --release                               # optimized build (LTO, single codegen unit)
cargo run -- serve --stdio                          # start LSP server over stdio
cargo run -- scan --dir /path/to/project            # run raw text scan on directory
```

### Linting & Formatting

```bash
cargo fmt                                            # format all code
cargo clippy --all-targets                          # lint (pedantic + custom rules enabled)
cargo clippy --fix --allow-dirty                    # auto-fix clippy warnings
cargo deny check                                     # audit dependencies (advisories, licenses, sources)
typos                                                # check for typos (configured in typos.toml)
```

### Testing

```bash
cargo test                                           # all unit and integration tests
cargo test -p anti-llm-cheat-lsp                    # unit tests only
cargo test -p anti-llm-cheat-lsp --test dogfood     # dogfood integration test (61 tests)
cargo test -p anti-llm-cheat-lsp -- --test-threads=1  # single-threaded run (for debugging)
cargo test rule_name                                # run tests matching pattern
```

Test fixtures live in `fixtures/`. Negative controls in `fixtures/negative_controls/` must each trigger at least one diagnostic. The dogfood suite runs end-to-end against fixture files, verifying detection surfaces and virtual-document rendering.

### Benchmarks

```bash
cargo bench                                          # run criterion benchmarks (output in target/criterion/report/index.html)
```

## Architecture

```
engine.rs           — AhoCorasick multi-format scanner (raw text, AST, manifest)
observations.rs     — raw observation types
diagnostics.rs      — AntiLlmDiagnostic → LSP Diagnostic conversion
server.rs           — AntiLlmServer: RulePackServer + LanguageServer impl (50k LOC)
capabilities.rs     — LSP 3.18 ServerCapabilities (matrix-derived, 95 methods)
config.rs           — centralized vocabulary (victory terms, forbidden patterns)
ast_adapter.rs      — RustAstAdapter wrapping AutoLspAdapter
semantic.rs         — AST-driven SemanticTokens
parsers/            — format-specific parsers (markdown claims, Rust AST, JSON-RPC, TOML)
rules/              — coverage matrices and detection rules (HEDGE-001, DEAD-ALT-001, etc.)
virtual_docs/       — virtual document modules (render fns per URI scheme)
ocel.rs             — OCEL 2.0 event-log writer for process-mining integration
```

### Multi-layer Detection Stack

1. **Raw Text Scanner** (engine.rs): Forbidaen victory-claim terms from config.rs, SemVer defaults ("1.0.0"), log-based routing. Uses Vec/String `.contains()` alongside pattern matching.
2. **Tree-Sitter AST Scanner**: Detects plain `tower-lsp` imports, namespace usage, unsafe code (`unwrap()`, `panic!()`), file mutations on read-only paths, string-shaped law checks.
3. **Cargo Manifest Parser**: Verifies no plain tower-lsp usage, enforces CalVer version laws.
4. **Markdown Claims Parser**: Checks docs for overclaim victory words or unverified route claims.
5. **JSON-RPC Transcript Parser**: Validates initialize capability transcripts for LSP 3.18 feature requests.
6. **Receipt JSON Validator**: Inspects BLAKE3-signed receipts to verify mutations have real admission proof.

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

No file I/O, no mutations. Virtual docs are computed from live state. See `virtual_docs/failset.rs` and `virtual_docs/lsp318_full_matrix.rs` for examples.

## Virtual Document URIs

Expose live state via these URIs:
- `anti-llm://failset` — live list of active blocking diagnostics
- `anti-llm://lsp318-matrix` — LSP 3.18 15-row delta changelog matrix (historical)
- `anti-llm://lsp318-full-matrix` — full 95-method combinatorial surface (authoritative)
- `anti-llm://lsif06-matrix` — full 38-element LSIF 0.6 surface
- `anti-llm://receipt-ledger` — rendered list of BLAKE3 receipts
- `anti-llm://forbidden-implications` — map of LLM overclaim prevention logic
- `anti-llm://checkpoint-status` — checkpoint verification status

## Diagnostic Codes

```
ANTI-LLM-TOWER-*     — plain tower-lsp reference
ANTI-LLM-VICTORY-*   — victory language in code/comments/docs
ANTI-LLM-CLAIMS-*    — overclaim in status words
ANTI-LLM-RECEIPT-*   — fake receipt (no boundary markers, no digest)
ANTI-LLM-ROUTE-*     — log-as-route-proof substitution
ANTI-LLM-VERSION-*   — CalVer violation (SemVer detected)
ANTI-LLM-DEAD-ALT-001  — dead alternative functions (_v2/_alt/_correct/_fixed/_working)
ANTI-LLM-HEDGE-001   — hedge comments admitting incomplete implementation
WASM4PM-*            — process-mining law violation (triggers gate ANDON)
GGEN-*               — ggen violation (triggers gate ANDON)
```

## Testing Patterns

### Unit Tests
Place module tests inline with `#[cfg(test)]` blocks. Test single functions in isolation.

### Integration Tests
Place in `tests/` directory (e.g., `tests/dogfood.rs`). Run end-to-end against fixture files.

### Fixtures
- `fixtures/` — test cases that should trigger diagnostics
- `fixtures/negative_controls/` — test cases that MUST trigger at least one diagnostic each
- Add new fixture files as `.rs`, `.md`, `.toml` files; dogfood suite auto-discovers them

### Dogfood Suite
Runs end-to-end server tests against fixtures, verifying detection surfaces, handler coverage, and virtual-document rendering. Run with:
```bash
cargo test --test dogfood
```

## Toolchain & Dependencies

- **Rust**: Pinned stable (nightly-2026-04-15) in `rust-toolchain.toml`
- **MSRV**: 1.82.0
- **Edition**: 2021
- **Key deps**: lsp-max (engine-bridge), tower-lsp, aho-corasick, tree-sitter, blake3, wasm4pm-compat
- **Local workspace deps**: star-toml, lsp-max (parent workspace)

Clippy lints are configured for safety: unsafe_code forbidden, unwrap_used warned, expect_used warned.

## Code Generation

Some types are generated. See `generated/` directory.

## Law Status

- Virtual doc `anti-llm://process-model`: CANDIDATE (renders live DFG + Declare)
- `RulePackServer` bridge: CANDIDATE
- `WorkspaceIndex` wiring: CANDIDATE
- LSP 3.18 coverage: PARTIAL (93/95 methods Wired or Refuses)
- LSIF 0.6 coverage: PARTIAL (all 38 elements modeled; no transcripts yet)
