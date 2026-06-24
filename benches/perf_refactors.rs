//! Admission benchmarks for the perf-refactors changeset.
//!
//! Four claims under measurement:
//!   B1 — OnceLock<Regex> eliminates per-call compilation overhead
//!   B2 — AhoCorasick single-pass scan outperforms 25× sequential contains()
//!   B3 — law_table() returning &'static [LawSpec] removes per-call Vec allocation
//!   B4 — RwLock concurrent reads are not serialized (no mutex contention on read path)
//!
//! Receipt written by `just bench-admit`. This file produces the measurements;
//! the receipt script hashes the output and stamps the boundary.

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use regex::Regex;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

// ── Shared corpus ─────────────────────────────────────────────────────────────

const CORPUS: &str = include_str!("../src/engine.rs");

// The 25 raw-smell patterns from engine.rs — duplicated here so the bench
// compiles independently of engine internals.
const PATTERNS: &[&str] = &[
    "tower-lsp",
    "tower_lsp",
    "CLAP",
    "Victory confirmed",
    "fully admitted",
    "all gaps resolved",
    "successfully proven",
    "Routing to PackPlan",
    "test result: ok",
    "v1.0.0",
    "version = \"1.0.0\"",
    "CLAP-DEBUG",
    "CLAP-DEBUG-PATH",
    "Content was:",
    "Path was:",
    "static scan as route proof",
    "static scan",
    "route proof",
    "ChangelogCoverage(15 rows) => SpecCoverage(LSP 3.18)",
    "ChangelogCoverage(15 rows) \u{21d2} SpecCoverage(LSP 3.18)",
    "15-row changelog matrix is being treated as full LSP 3.18 combinatorial coverage",
    "ANTI-LLM-OCEL-001-TRIGGER",
    "ANTI-LLM-OCEL-002-TRIGGER",
    "\"bypassed_compat\": true",
    "use wasm4pm::",
];

// ── B1: OnceLock<Regex> vs fresh Regex::new() ────────────────────────────────

fn fresh_regex_compile(_input: &str) -> usize {
    let re = Regex::new(r"(?i)\b(tower-lsp|tower_lsp|CLAP|Victory confirmed)\b").unwrap();
    re.find_iter(_input).count()
}

fn onclock_regex(input: &str) -> usize {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)\b(tower-lsp|tower_lsp|CLAP|Victory confirmed)\b").unwrap()
    });
    re.find_iter(input).count()
}

fn bench_b1_regex_onclock(c: &mut Criterion) {
    let mut group = c.benchmark_group("B1_regex_compilation");
    group.throughput(Throughput::Bytes(CORPUS.len() as u64));

    group.bench_function("fresh_compile_per_call", |b| {
        b.iter(|| fresh_regex_compile(black_box(CORPUS)))
    });

    group.bench_function("onclock_compiled_once", |b| {
        b.iter(|| onclock_regex(black_box(CORPUS)))
    });

    group.finish();
}

// ── B2: AhoCorasick single-pass vs 25× contains() ───────────────────────────

fn sequential_contains(input: &str) -> usize {
    PATTERNS.iter().filter(|&&p| input.contains(p)).count()
}

fn aho_corasick_scan(input: &str) -> usize {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    let ac = AC.get_or_init(|| {
        AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build(PATTERNS)
            .unwrap()
    });
    ac.find_iter(input).count()
}

fn bench_b2_aho_corasick(c: &mut Criterion) {
    let mut group = c.benchmark_group("B2_multi_pattern_scan");
    group.throughput(Throughput::Bytes(CORPUS.len() as u64));

    group.bench_function("sequential_contains_25x", |b| {
        b.iter(|| sequential_contains(black_box(CORPUS)))
    });

    group.bench_function("aho_corasick_single_pass", |b| {
        b.iter(|| aho_corasick_scan(black_box(CORPUS)))
    });

    group.finish();
}

// ── B3: Static slice vs per-call Vec allocation ──────────────────────────────

#[derive(Clone)]
struct LawEntry {
    #[allow(dead_code)]
    id: &'static str,
    gate_id: &'static str,
}

fn law_table_alloc() -> Vec<LawEntry> {
    vec![
        LawEntry {
            id: "diag-uninitialized",
            gate_id: "gate-state-check",
        },
        LawEntry {
            id: "diag-missing-receipt",
            gate_id: "gate-receipt-check",
        },
        LawEntry {
            id: "diag-auth-violation",
            gate_id: "gate-auth-check",
        },
        LawEntry {
            id: "diag-powl-conformance",
            gate_id: "gate-powl-conformance",
        },
        LawEntry {
            id: "diag-snapshot-drift",
            gate_id: "gate-snapshot-check",
        },
    ]
}

fn law_table_static() -> &'static [LawEntry] {
    static TABLE: OnceLock<Vec<LawEntry>> = OnceLock::new();
    TABLE.get_or_init(|| {
        vec![
            LawEntry {
                id: "diag-uninitialized",
                gate_id: "gate-state-check",
            },
            LawEntry {
                id: "diag-missing-receipt",
                gate_id: "gate-receipt-check",
            },
            LawEntry {
                id: "diag-auth-violation",
                gate_id: "gate-auth-check",
            },
            LawEntry {
                id: "diag-powl-conformance",
                gate_id: "gate-powl-conformance",
            },
            LawEntry {
                id: "diag-snapshot-drift",
                gate_id: "gate-snapshot-check",
            },
        ]
    })
}

fn bench_b3_law_table(c: &mut Criterion) {
    let mut group = c.benchmark_group("B3_law_table_allocation");

    group.bench_function("vec_alloc_per_call", |b| {
        b.iter(|| {
            let table = law_table_alloc();
            black_box(
                table
                    .iter()
                    .filter(|e| e.gate_id.starts_with("gate"))
                    .count(),
            )
        })
    });

    group.bench_function("static_slice_onclock", |b| {
        b.iter(|| {
            let table = law_table_static();
            black_box(
                table
                    .iter()
                    .filter(|e| e.gate_id.starts_with("gate"))
                    .count(),
            )
        })
    });

    group.finish();
}

// ── B4: parking_lot::RwLock vs std::sync::Mutex for concurrent reads ─────────
//
// std::sync::RwLock uses pthread_rwlock_t on macOS which has higher syscall
// overhead than Mutex under concurrent reads. parking_lot::RwLock is
// user-space with a fast path for uncontested reads.

fn bench_b4_rwlock_vs_mutex(c: &mut Criterion) {
    const READER_THREADS: usize = 8;
    const OPS_PER_THREAD: usize = 1_000;

    let mut group = c.benchmark_group("B4_concurrent_read_lock");

    group.bench_with_input(
        BenchmarkId::new("std_mutex_serialized_reads", READER_THREADS),
        &READER_THREADS,
        |b, &n| {
            let map: Arc<Mutex<Vec<String>>> =
                Arc::new(Mutex::new((0..100).map(|i| format!("doc-{i}")).collect()));
            b.iter(|| {
                let handles: Vec<_> = (0..n)
                    .map(|_| {
                        let m = Arc::clone(&map);
                        thread::spawn(move || {
                            for _ in 0..OPS_PER_THREAD {
                                let guard = m.lock().unwrap();
                                black_box(guard.len());
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("parking_lot_rwlock_reads", READER_THREADS),
        &READER_THREADS,
        |b, &n| {
            let map: Arc<parking_lot::RwLock<Vec<String>>> = Arc::new(parking_lot::RwLock::new(
                (0..100).map(|i| format!("doc-{i}")).collect(),
            ));
            b.iter(|| {
                let handles: Vec<_> = (0..n)
                    .map(|_| {
                        let m = Arc::clone(&map);
                        thread::spawn(move || {
                            for _ in 0..OPS_PER_THREAD {
                                let guard = m.read();
                                black_box(guard.len());
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
            });
        },
    );

    group.finish();
}

// ── B5: Line index vs repeated lines().count() ───────────────────────────────

fn line_count_naive(content: &str, offsets: &[usize]) -> Vec<usize> {
    offsets
        .iter()
        .map(|&idx| content[..idx].lines().count() + 1)
        .collect()
}

fn line_count_indexed(content: &[u8], offsets: &[usize]) -> Vec<usize> {
    // Build index once with SIMD memchr, then binary-search per offset.
    let mut line_starts = Vec::with_capacity(content.len() / 40 + 1);
    line_starts.push(0usize);
    for pos in memchr::memchr_iter(b'\n', content) {
        line_starts.push(pos + 1);
    }
    offsets
        .iter()
        .map(|&byte| line_starts.partition_point(|&s| s <= byte))
        .collect()
}

fn bench_b5_line_index(c: &mut Criterion) {
    let content = CORPUS;
    let bytes = content.as_bytes();
    // Simulate 20 pattern match positions spread across the file.
    // Snap each offset to the nearest valid char boundary.
    let step = content.len() / 20;
    let offsets: Vec<usize> = (0..20)
        .map(|i| {
            let mut off = i * step;
            while off < content.len() && !content.is_char_boundary(off) {
                off += 1;
            }
            off.min(content.len())
        })
        .collect();

    let mut group = c.benchmark_group("B5_line_number_lookup");
    group.throughput(Throughput::Elements(offsets.len() as u64));

    group.bench_function("naive_lines_count_per_match", |b| {
        b.iter(|| line_count_naive(black_box(content), black_box(&offsets)))
    });

    group.bench_function("indexed_memchr_binary_search", |b| {
        b.iter(|| line_count_indexed(black_box(bytes), black_box(&offsets)))
    });

    group.finish();
}

// ── B6: FxHashMap vs std::HashMap for short string keys ──────────────────────

fn bench_b6_fxhashmap(c: &mut Criterion) {
    use rustc_hash::FxHashMap;
    use std::collections::HashMap;

    let methods: Vec<&'static str> = vec![
        "initialize",
        "initialized",
        "shutdown",
        "textDocument/didOpen",
        "textDocument/didChange",
        "textDocument/didClose",
        "textDocument/hover",
        "textDocument/codeAction",
        "textDocument/diagnostic",
        "workspace/symbol",
        "max/snapshot",
        "max/conformanceVector",
        "max/release",
        "max/receipt",
    ];

    let mut group = c.benchmark_group("B6_hashmap_method_lookup");

    group.bench_function("std_hashmap_lookup", |b| {
        let map: HashMap<&str, usize> = methods.iter().enumerate().map(|(i, &m)| (m, i)).collect();
        b.iter(|| {
            for &method in &methods {
                black_box(map.get(black_box(method)));
            }
        })
    });

    group.bench_function("fxhashmap_lookup", |b| {
        let map: FxHashMap<&str, usize> =
            methods.iter().enumerate().map(|(i, &m)| (m, i)).collect();
        b.iter(|| {
            for &method in &methods {
                black_box(map.get(black_box(method)));
            }
        })
    });

    group.finish();
}

// ── Harness ───────────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_b1_regex_onclock,
    bench_b2_aho_corasick,
    bench_b3_law_table,
    bench_b4_rwlock_vs_mutex,
    bench_b5_line_index,
    bench_b6_fxhashmap,
);
criterion_main!(benches);
