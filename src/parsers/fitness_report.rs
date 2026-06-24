use crate::observations::Observation;

pub fn parse_fitness_report(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    let v: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return obs,
    };

    let fitness = v.get("fitness").and_then(|f| f.as_f64()).unwrap_or(0.0);
    let admitted = v.get("admitted").and_then(|a| a.as_bool()).unwrap_or(false);
    let has_provenance = v.get("provenance").is_some();
    let top_level_run_id = v.get("run_id").is_some();
    let provenance_run_id = v.get("provenance").and_then(|p| p.get("run_id")).is_some();
    let has_run_id = top_level_run_id || provenance_run_id;

    // ADMIT-001: fitness=1.0 + admitted=true but no provenance block
    if (fitness - 1.0).abs() < f64::EPSILON && admitted && !has_provenance {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: 0,
            end_byte: content.len(),
            line: 1,
            column: 1,
            kind: "fitness_report".to_string(),
            construct: "fitness_bare_constant".to_string(),
            context: format!("fitness={}, admitted={}, provenance=absent", fitness, admitted),
            message: "Fitness report asserts 1.0/admitted without measurement provenance block — A10 premature admission".to_string(),
        });
    }

    // ADMIT-003: admitted=true without run_id
    if admitted && !has_run_id {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: 0,
            end_byte: content.len(),
            line: 1,
            column: 1,
            kind: "fitness_report".to_string(),
            construct: "admitted_no_run_id".to_string(),
            context: format!("admitted={}, run_id=absent", admitted),
            message: "Fitness report admits breed without run_id in provenance — cannot trace back to a measured run".to_string(),
        });
    }

    obs
}
