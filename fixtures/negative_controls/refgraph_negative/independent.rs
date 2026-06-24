// Negative control for cross-product #4: a symbol that does NOT depend on the
// unwitnessed one. The seed `oracle_metric` is present in the same scan, but
// `independent_pure` and its caller `independent_caller` form a separate
// component with NO reference edge reaching the seed. They must NEVER enter the
// failset — no false transitive ANDON.

// @unwitnessed: oracle_metric

pub fn oracle_metric() -> f64 {
    0.847
}

// Separate component — references only `add_one`, never `oracle_metric`.
pub fn add_one(x: i64) -> i64 {
    x + 1
}

pub fn independent_pure(x: i64) -> i64 {
    let y = add_one(x);
    y * 2
}

pub fn independent_caller() -> i64 {
    independent_pure(41)
}
