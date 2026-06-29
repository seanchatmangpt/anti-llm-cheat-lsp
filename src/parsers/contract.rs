use std::collections::{HashMap, HashSet};

use crate::observations::Observation;

/// Extract breed_id from a file path containing `breeds/<breed_id>`.
fn extract_breed_id(path: &str) -> Option<&str> {
    let idx = path.find("breeds/")?;
    let after = &path[idx + "breeds/".len()..];
    let end = after.find('/').or_else(|| after.find(".rs")).unwrap_or(after.len());
    Some(&after[..end])
}

fn is_src_path(path: &str) -> bool {
    (path.contains("/src/") || path.contains("src/breeds")) && !path.contains("tests/")
}

fn is_test_path(path: &str) -> bool {
    path.contains("tests/")
        || path.ends_with("_test.rs")
        || path.contains("/test/")
        || path.contains("fixtures/")
        || path.contains("benches/")
        || path.contains("examples/")
        || path.contains("build.rs")
}

/// Cross-file contract schism detection (A9).
///
/// Groups fn_definition observations by breed_id and compares the set of
/// function names in `src/breeds/<b>.rs` vs oracle test files for that breed.
pub fn detect_contract_schism(all_obs: &[Observation]) -> Vec<Observation> {
    let mut obs = Vec::new();

    // Collect fn_definition observations
    let fn_defs: Vec<&Observation> = all_obs.iter().filter(|o| o.kind == "fn_definition").collect();

    // Group by breed_id × (src vs test)
    let mut breed_src_fns: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut breed_test_fns: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut breed_src_path: HashMap<&str, &str> = HashMap::new();

    for o in &fn_defs {
        if let Some(breed_id) = extract_breed_id(&o.file_path) {
            if is_src_path(&o.file_path) {
                breed_src_fns.entry(breed_id).or_default().insert(&o.construct);
                breed_src_path.entry(breed_id).or_insert(&o.file_path);
            } else if is_test_path(&o.file_path) {
                breed_test_fns.entry(breed_id).or_default().insert(&o.construct);
            }
        }
    }

    // CONTRACT-001: zero function name overlap between impl and oracle test
    for (breed_id, src_fns) in &breed_src_fns {
        if let Some(test_fns) = breed_test_fns.get(breed_id) {
            let overlap: HashSet<&&str> = src_fns.intersection(test_fns).collect();
            // Only flag if BOTH sides have substantial functions and ZERO overlap
            if overlap.is_empty() && src_fns.len() >= 3 && test_fns.len() >= 3 {
                let file_path = breed_src_path.get(breed_id).copied().unwrap_or("unknown");
                obs.push(Observation {
                    file_path: file_path.to_string(),
                    start_byte: 0,
                    end_byte: 0,
                    line: 1,
                    column: 1,
                    kind: "contract_schism".to_string(),
                    construct: "contract_vocab_divergence".to_string(),
                    context: breed_id.to_string(),
                    message: format!(
                        "Breed '{}': zero function name overlap between src ({} fns) and oracle test ({} fns) — A9 contract schism",
                        breed_id, src_fns.len(), test_fns.len()
                    ),
                });
            }
        }
    }

    // CONTRACT-002: same function name defined in BOTH src and test for the same breed
    for (breed_id, src_fns) in &breed_src_fns {
        if let Some(test_fns) = breed_test_fns.get(breed_id) {
            let shadows: Vec<&&str> = src_fns.intersection(test_fns).collect();
            // If a non-trivial named fn appears in both (not just "run" which is expected in trait)
            for &shadow_fn in &shadows {
                if !matches!(*shadow_fn, "run" | "new" | "default") {
                    let file_path = breed_src_path.get(breed_id).copied().unwrap_or("unknown");
                    obs.push(Observation {
                        file_path: file_path.to_string(),
                        start_byte: 0,
                        end_byte: 0,
                        line: 1,
                        column: 1,
                        kind: "contract_schism".to_string(),
                        construct: "contract_fn_shadow".to_string(),
                        context: format!("breed={} fn={}", breed_id, shadow_fn),
                        message: format!(
                            "Breed '{}': function '{}' defined in BOTH src and test — shadow override cheat (A9)",
                            breed_id, shadow_fn
                        ),
                    });
                }
            }
        }
    }

    obs
}
