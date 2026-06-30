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

    // ADMIT-004: fabricated run_id
    let run_id_str = v
        .get("run_id")
        .or_else(|| v.get("provenance").and_then(|p| p.get("run_id")))
        .and_then(|r| r.as_str())
        .unwrap_or("");
    if !run_id_str.is_empty() {
        let is_zero_uuid = run_id_str == "00000000-0000-0000-0000-000000000000";
        let is_sequential = run_id_str.starts_with("12345678-");
        let is_placeholder_string = run_id_str.starts_with("test-run-")
            || run_id_str.starts_with("conformance-run-")
            || run_id_str == "run-001"
            || run_id_str.starts_with("fake-run");
        if is_zero_uuid || is_sequential || is_placeholder_string {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: 0,
                end_byte: content.len(),
                line: 1,
                column: 1,
                kind: "fitness_report".to_string(),
                construct: "fabricated_run_id".to_string(),
                context: format!("run_id={}", run_id_str),
                message: format!(
                    "Fitness report contains fabricated run_id '{}' — placeholder, not a real conformance execution",
                    run_id_str
                ),
            });
        }
    }

    // ADMIT-005: fitness >= 0.95 + admitted=true but provenance lacks measured_by
    let provenance_has_measured_by = v
        .get("provenance")
        .and_then(|p| p.get("measured_by"))
        .is_some();
    if fitness >= 0.95 && admitted && !provenance_has_measured_by {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: 0,
            end_byte: content.len(),
            line: 1,
            column: 1,
            kind: "fitness_report".to_string(),
            construct: "admitted_no_measured_by".to_string(),
            context: format!(
                "fitness={:.3}, admitted={}, provenance.measured_by=absent",
                fitness, admitted
            ),
            message: "Fitness admitted at >=0.95 but provenance block lacks 'measured_by' — admission cannot be attributed to a tool or agent".to_string(),
        });
    }

    obs
}
