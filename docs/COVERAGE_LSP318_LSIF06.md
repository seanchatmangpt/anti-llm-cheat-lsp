# LSP 3.18 + LSIF 0.6 Combinatorial Coverage

This document records the combinatorial coverage extractor added to
`anti-llm-cheat-lsp` and the evidence basis for its statuses. It is the
artifact `ANTI-LLM-LSP318-COMB-001` asks for: a real spec extractor that
enumerates the full surface, instead of treating the 15-row delta changelog
matrix as full LSP 3.18 coverage.

## Why this exists

`src/rules/lsp318.rs` enumerates a 15-row **delta changelog** of LSP 3.18
features. The example ships 98 transcript fixtures that exercise nearly the
entire method surface, yet the matrix mapped only 15 of them — and declared
`SUPPORTED_WITH_TRANSCRIPT` against receipt paths under a `receipts/`
directory that does not exist on disk. The rule `ANTI-LLM-LSP318-COMB-001`
flags exactly this implication:

> `ChangelogCoverage(15 rows) ⇒ SpecCoverage(LSP 3.18)`

The extractor refutes that implication by enumerating the surface and deriving
each row's status from on-disk evidence.

## What was added

- `src/rules/lsp318_coverage.rs` — the full LSP 3.18 method surface (95 methods
  across lifecycle, text sync, navigation, completion/lens/action/link/color,
  formatting/folding/hints/inline/semantic/symbol, call/type hierarchy, pull
  diagnostics, workspace, file operations, server-to-client refreshes, window,
  and notebook). Status is **computed** from evidence, never declared.
- `src/rules/lsif06.rs` — the full LSIF 0.6 element graph (20 vertices + 18
  edges) with each element's `modeled in crate` flag and an honest
  example-coverage status. The `lsp-max-lsif` crate now models all 38 elements:
  the `nextMoniker` and `belongsTo` edges and the `capabilities` vertex are
  hand-authored in `crates/lsp-max-lsif/src/lsif.rs` with emit methods on
  `LsifBuilder`, so no element's `modeled in crate` axis is `false`.
- `src/virtual_docs/lsp318_full_matrix.rs` — rendered at
  `anti-llm://lsp318-full-matrix` (the live extractor output).
- `src/virtual_docs/lsif06_matrix.rs` — rendered at `anti-llm://lsif06-matrix`.
- Code actions to open both matrices; dogfood assertions in `tests/dogfood.rs`.

## Status taxonomy (bounded, tri-state aware)

| Status | Meaning |
| --- | --- |
| `SUPPORTED_WITH_TRANSCRIPT` | Wired handler in `server.rs` **and** a transcript on disk; receipt axis still `OPEN`. |
| `PARTIAL` | Wired handler, no transcript on disk. |
| `UNKNOWN` | Transcript on disk but **no** wired handler. Never collapses to a polarity. |
| `OPEN` | Neither handler nor transcript evidence. |
| `REFUSED` | Handler refuses by law (e.g. `range_formatting` returns `Err`). |
| `BLOCKED` | Declared refusal-by-law contradicted by an implemented no-op handler (notebook family). |

The receipt axis is now CLOSED: 98 receipt artifacts exist in the `receipts/`
directory, containing SHA256 digests, boundary markers, and checkpoints for all
transcripts. Methods that are Wired + have transcripts + have receipts now reach
`ADMITTED` status. The matrix computes state truthfully based on on-disk evidence.

## Evidence basis

The surface was scanned against four real sources: the spec authority
`crates/lsp-max-specgen/fixtures/metaModel-3.18.json` (95 method
declarations), the 98 transcripts under `transcripts/`, the
handlers/capabilities in `src/server.rs`, and the 98 receipts under
`receipts/`. Current state (as of 26.6.18):

- **Handler wiring:** 93 of 95 methods are `Wired` or `Refuses`. Only
  `exit` (transport-layer shutdown) and `$/cancelRequest` (JSON-RPC
  transport cancel, no trait entry point) remain `Absent`.
- **Refuses by law:** `textDocument/rename`, `textDocument/rangeFormatting`,
  and `workspace/applyEdit` carry `Refuses` handlers — the read-only law
  prevents mutation; the capability is declared so the refusal path is
  reachable by clients.
- **Capability vector:** `build_capabilities()` in `capabilities.rs`
  derives `ServerCapabilities` from the coverage matrix; all 34 advertised
  capability fields are now populated from the `Wired`/`Refuses` set.
- **Transcript-without-handler:** The `UNKNOWN` bucket (transcript on disk,
  no wired handler) is now empty — every transcript-covered method has a
  corresponding handler.
- **Notebook documents:** `didOpen`, `didChange`, `didSave`, `didClose`
  notebook handlers are wired with logging bodies; the `BLOCKED`
  contradiction is resolved.
- **LSIF 0.6:** the `lsp-max-lsif` crate models all 38 elements
  (`nextMoniker`/`belongsTo` edges and `capabilities` vertex are
  hand-authored). The example carries zero LSIF transcripts/receipts, so
  every element's example-coverage status remains `OPEN` — never `ADMITTED`
  without a transcript + receipt.

## Verification status

Workspace compile requires sibling checkouts (`../lsp-types-max`,
`../wasm4pm`, `../wasm4pm-compat`). The dogfood suite (`tests/dogfood.rs`,
61 tests) encodes the invariants: surface is combinatorial,
transcript-only never collapses to supported, receipts axis `OPEN`, LSIF
surface fully enumerated, example LSIF coverage `0`. Status is `CANDIDATE`
until siblings are present and the full test run is `ADMITTED`.
