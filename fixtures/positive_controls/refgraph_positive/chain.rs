// Positive control for cross-product #4: a genuine transitive dependent chain.
//
// `oracle_metric` is declared unwitnessed. `compute_fitness` calls it directly
// (depth 1) and `run_gate` calls `compute_fitness` (depth 2). Both MUST appear
// in the bounded failset (REFGRAPH-001).

// @unwitnessed: oracle_metric

pub fn oracle_metric() -> f64 {
    // pretend-derived metric with no witness
    0.847
}

pub fn compute_fitness() -> f64 {
    let m = oracle_metric();
    m * 1.0
}

pub fn run_gate() -> bool {
    let f = compute_fitness();
    f > 0.5
}
