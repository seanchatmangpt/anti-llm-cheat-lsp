# Path C: RulePackServer Migration — Architectural Trade-Off Analysis

## Executive Summary

Path C proposes migrating anti-llm-cheat-lsp to the `RulePackServer` trait, which would auto-provide the entire LSP lifecycle (did_open/change/close, diagnostics, conformance vectors) in exchange for expressing the detection engine as a set of `Rule` structs loaded from TOML files.

**Verdict: LOSSY. Not recommended for current anti-llm-cheat-lsp.**

The engine's detection power would degrade because RulePackServer assumes:
- Single-language regex-pattern matching
- Per-file scope (no cross-file state)
- Simple pattern matching (regex, not AST-aware)

Current anti-llm-cheat-lsp uses:
- Multi-format parsing (Rust, TOML, Python, TypeScript, etc.)
- Aho-Corasick for efficient multi-pattern simultaneous detection
- Cross-file law validation (e.g., "every RULE-ID defined must be referenced")
- Stateful analysis (receipt digests, law-axis tracing, conformance scoring)

---

## What RulePackServer Provides (ERRC Innovations)

### 1. **RulePackSnapshot** ✓
- Immutable `Arc`-wrapped workspace state cloneable into async dispatch
- Mirrors rust-analyzer's `GlobalStateSnapshot` pattern
- *Anti-llm-cheat-lsp today uses:* Arc<Mutex<>> for workspace_root; could adopt this

### 2. **Rule-pack composition** ✓
- Dependency-resolved ordering of packs with version compatibility checks
- Conflict detection between packs
- *Anti-llm-cheat-lsp today:* Single monolithic engine; no pack composition

### 3. **Latency classification (EvalBudget)** ✓
- Sync rules (< 50 ms) evaluated inline in did_open/did_change
- Background rules dispatched to Tokio tasks
- Dynamic reclassification based on SPC monitoring
- *Anti-llm-cheat-lsp today:* Synchronous scanning on did_* events; no background tasks

### 4. **Workspace-wide cross-file diagnostics** ✓
- `WorkspaceIndex` tracks all open documents and per-file conformance vectors
- Cross-file rules can emit diagnostics on file A based on content in file B
- Workspace-level conformance vector is the aggregate across all files
- *Anti-llm-cheat-lsp today:* Cross-file engine exists; not wired to LSP workspace index

---

## What RulePackServer Assumes

### Constraint 1: Single-language regex-based pattern matching
```rust
pub struct Rule {
    pub pattern: String,  // Rust regex, not AST query language
    pub path_globs: Vec<String>,
    pub exclude_globs: Vec<String>,
    // ...
}
```

**Current anti-llm-cheat-lsp violations:**

1. **Multi-format parsing**
   - Detects violations in Rust code (AST-based, aho-corasick)
   - Detects violations in TOML (custom parser, ggen ontology validation)
   - Detects violations in Python (custom parser)
   - Detects violations in TypeScript (custom parser)
   - **Migration cost:** Express each format as regex patterns (lossy; AST-aware checks disappear)

2. **Aho-Corasick multi-pattern matching**
   - Current engine uses `aho_corasick::AhoCorasick` to match multiple patterns simultaneously in a single pass
   - More efficient and expressive than individual regex matches
   - **Migration cost:** Break into separate `Rule` entries; linear scan per rule (performance regression)

3. **Stateful analysis**
   - Engine maintains per-file state: receipt digests, law-axis tracing, conformance axes
   - **Migration cost:** Regex patterns cannot maintain state; complex analyses become impossible

---

### Constraint 2: Per-file scope only (WorkspaceIndex optional)

RulePackServer's cross-file model is:
```rust
pub struct CrossFileRule {
    pub source_glob: String,        // Find matches in these files
    pub source_pattern: String,     // This pattern
    pub target_glob: String,        // Must exist in these files
    pub target_pattern: String,     // This pattern
}
```

**Current anti-llm-cheat-lsp violations:**

1. **Shadowing dependencies**
   - Detects when a utility function is called but its definition is hidden (ggen ontology validation)
   - Requires semantic understanding of Rust scoping
   - **Migration cost:** Cannot express with regex patterns over files; would require custom rules

2. **Receipt chain validation**
   - Verifies BLAKE3 digest chains across receipt artifacts
   - Checks boundary markers and checkpoint closure
   - **Migration cost:** Regex patterns cannot validate cryptographic digests or parse structured data

3. **Conformance vector law axes**
   - Traces which law axis is refusing admission (transcript missing? negative control missing?)
   - Builds per-axis state machines
   - **Migration cost:** Regex patterns are stateless; law-axis tracing disappears

---

### Constraint 3: Simple pattern matching (no AST queries)

RulePackServer uses:
```rust
let re = Regex::new(&rule.pattern)?;
for (line_idx, line) in content.lines().enumerate() {
    for mat in re.find_iter(line) {
        // emit diagnostic
    }
}
```

**Current anti-llm-cheat-lsp violations:**

1. **Tree-sitter AST queries**
   - Detects "node is missing" in tree-sitter parse tree
   - Detects syntax errors by analyzing AST structure
   - **Migration cost:** AST queries are more powerful than regex; must downgrade to pattern matching

2. **Multi-line pattern matching**
   - Some detections require understanding context across multiple lines
   - **Migration cost:** Regex can cross lines, but becomes complex and error-prone

3. **Ggen ontology validation**
   - Custom domain-specific language for process-mining ontologies
   - Cannot be expressed as regex patterns
   - **Migration cost:** Lose custom parser entirely; revert to surface-level regex

---

## Concrete Example: Tower-LSP Detection

### Current Implementation
```rust
// Multi-pass analysis:
1. Scan for plain "tower-lsp" or "tower_lsp" references (regex)
2. Check if it's in a `cfg(test)` block (AST-aware)
3. Check if it's a negative-control fixture (cross-file semantics)
4. Validate with receipt chain (cryptographic validation)
// Each step eliminates false positives and provides law-axis feedback
```

### RulePackServer Migration
```toml
[[rules]]
id = "ANTI-LLM-TOWER-LSP"
pattern = "tower[-_]lsp"          # Matches everything; no filtering
message = "plain tower-lsp reference detected"
# No way to:
# - Filter for test-only blocks (AST-aware)
# - Validate negative-control status (cross-file)
# - Check receipt chain (stateful)
# Result: False positives + no law-axis feedback
```

---

## Migration Path (If Pursued)

### Phase 1: Express simple detections as rules
- Victory language detection ("solved", "done", "fully admitted")
- Basic token matching (CLAP_*, "_shim", "_facade")
- ~20 rules, no multi-format parsing needed

**Lost capability:** ~40 sophisticated detections

### Phase 2: Attempt to express complex detections
- Create complex regex patterns for each violation
- Use CrossFileRule for some dependency checks
- Maintain ad-hoc stateful analysis outside RulePackServer

**Hybrid approach:** Defeats the purpose (boilerplate not eliminated)

---

## Why Anti-llm-cheat-lsp Should NOT Migrate

1. **Sophisticated detection engine** — aho-corasick, multi-format parsing, AST analysis are baked into the core mission
2. **Stateful analysis** — law-axis tracing and receipt validation require maintaining state
3. **Cross-file semantics** — ggen ontology validation and shadowing checks need semantic understanding
4. **Performance-critical** — aho-corasick is more efficient than regex for 50+ simultaneous patterns
5. **Law compliance** — migrating to regex-only patterns would violate AGENTS.md laws around receipt validation

---

## Why RulePackServer IS Valuable

For servers with simpler missions:
- **pattern-lsp** — pure regex-based linting (good fit)
- **axum-lsp** — route and decorator validation (good fit)
- **clap-noun-verb-lsp** — CLI schema checking (good fit)
- **Anti-cheating servers** that don't need cross-file or multi-format analysis (good fit)

RulePackServer eliminates boilerplate for regex-only servers and provides:
- Automatic LSP lifecycle management
- Conformance vector building
- Workspace-level cross-file rule support (when needed)
- Latency classification and SPC monitoring

---

## Recommendation

### Keep anti-llm-cheat-lsp as-is (Paths A + B sufficient)
- Path A (capabilities codegen) ✓ IMPLEMENTED
- Path B (AST adapter) ✓ IMPLEMENTED
- Path C would be a regression; don't migrate

### Use RulePackServer for next-generation servers
- If a new LSP server needs only regex-based detection, use RulePackServer as the base
- If a server needs AST analysis or cross-file semantics, extend lsp-max directly

---

## Conclusion

Path C demonstrates that **not all LSP servers should use the same base trait**. RulePackServer is an excellent fit for rule-pack-based servers, but anti-llm-cheat-lsp's sophisticated detection engine would be compromised by forced migration. Paths A and B provide high-value improvements without sacrificing detection power.

**Final verdict: Paths A + B deliver > Path C. Stop at Path B.**
