# Anti-LLM Admissibility Canary LSP Server

`anti-llm-cheat-lsp` is a specialized LSP server proving ground and admissibility verification canary built on `lsp-max` (LSP 3.18). It detects common LLM-generated code patterns, unverified claims, and incorrect routing abstractions, demonstrating the enforcement of inverted LSP laws within a development environment.

## How the Canary Works

The canary monitors files and uses a multi-layered detector stack to produce diagnostics and enforce safety checkpoints.

### The Detector Stack

1. **Raw Text Scanner:** Detects forbidden victory-claim terms from the centralized vocabulary in `config.rs` (e.g., "Victory confirmed", "fully admitted"), template SemVer defaults ("1.0.0"), and log-based routing indicators ("Routing to PackPlan"). Vec/String `.contains()` misuse is detected and diagnosed separately from pattern-match paths.
2. **Tree-Sitter AST Scanner:** Traverses Rust source code to detect plain `tower-lsp` imports, namespace usage (`tower_lsp::`), unsafe code smells (`unwrap()`, `panic!()`), direct file mutation attempts on read-only paths (`std::fs::write`, `File::create`), and string-shaped matching for law checks.
3. **Cargo Manifest Parser:** Verifies `Cargo.toml` and `Cargo.lock` to ensure plain `tower-lsp` is not used and that CalVer version laws are enforced.
4. **Markdown Claims Parser:** Checks markdown documentation for overclaim victory words or unverified route claims.
5. **JSON-RPC Transcript Parser:** Validates initialize capability transcripts to verify that client capabilities explicitly request LSP 3.18 features rather than relying on plain LSP fallback.
6. **Receipt JSON Validator:** Inspects BLAKE3 cryptographically signed receipts to verify that mutations are accompanied by real admission proof.

### Rule Index

| Rule ID | What it detects |
|---------|-----------------|
| ANTI-LLM-DEAD-ALT-001 | Dead alternative functions (`_v2`/`_alt`/`_correct`/`_fixed`/`_working`) defined but never called |
| ANTI-LLM-HEDGE-001 | Hedge comments admitting incomplete implementation ("In a real implementation…", "For now…") |

### Centralized Victory Vocabulary

All forbidden victory-claim terms are defined in `src/config.rs` as the single source of truth. The engine (`engine.rs`) and all rule modules reference this list — no rule hand-rolls its own term list. This prevents drift where one rule detects "done" but another silently misses "fully admitted".

## LSP 3.18 Detection Surface and Virtual Documents

The canary wires **93 of 95 LSP 3.18 methods** as detection surfaces
(`Wired` handler state). The remaining 2 — `exit` and `$/cancelRequest` —
are handled at the transport layer with no `LanguageServer` trait entry
point. `workspace/applyEdit` is `Refuses` by the read-only law.

Coverage is **computed from evidence** (on-disk handlers, transcripts,
receipts), never declared. Status values follow the bounded taxonomy:
`SUPPORTED_WITH_TRANSCRIPT`, `PARTIAL`, `UNKNOWN`, `OPEN`, `REFUSED`,
`BLOCKED`. See
[`docs/COVERAGE_LSP318_LSIF06.md`](docs/COVERAGE_LSP318_LSIF06.md) for
the full matrix and evidence basis.

The LSIF 0.6 element graph (38 elements: 20 vertices + 18 edges) is fully
modeled in `crates/lsp-max-lsif`; example-coverage status is `OPEN`
(modeled substrate, no transcripts or receipts produced by this canary).

Virtual documents expose live state:

* `anti-llm://failset` — live list of active blocking diagnostics.
* `anti-llm://lsp318-matrix` — LSP 3.18 15-row delta changelog matrix (historical).
* `anti-llm://lsp318-full-matrix` — full 95-method combinatorial surface (authoritative).
* `anti-llm://lsif06-matrix` — full 38-element LSIF 0.6 surface.
* `anti-llm://receipt-ledger` — rendered list of BLAKE3 receipts.
* `anti-llm://forbidden-implications` — map of LLM overclaim prevention logic.
* `anti-llm://checkpoint-status` — checkpoint verification status.

## Usage

### Run Directory Scan
To run a raw text scan over a directory:
```bash
cargo run --package anti-llm-cheat-lsp -- scan --dir /path/to/project
```

### Start the LSP Server
Start the stdio-based LSP server:
```bash
cargo run --package anti-llm-cheat-lsp -- serve --stdio
```

## Running Tests

Run the dogfood suite (61 tests) to verify detection surfaces, handler coverage,
and virtual-document rendering:

```bash
cargo test --package anti-llm-cheat-lsp
cargo test -p anti-llm-cheat-lsp --test dogfood
```
