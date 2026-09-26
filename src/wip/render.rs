use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::model::{WipReport, WIP_REPORT_SCHEMA};

impl WipReport {
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Little's Law WIP Closure Report\n\n");
        out.push_str(&format!("Status: **{:?}**\n\n", self.status).to_uppercase());
        out.push_str("## Flow metrics\n\n");
        out.push_str("| Metric | Observed value |\n| --- | ---: |\n");
        out.push_str(&format!("| WIP L | {} |\n", self.metrics.wip_l));
        out.push_str(&format!(
            "| Throughput λ/day | {} |\n",
            fmt_opt(self.metrics.throughput_lambda_per_day)
        ));
        out.push_str(&format!(
            "| Mean cycle time W (days) | {} |\n",
            fmt_opt(self.metrics.mean_cycle_time_days)
        ));
        out.push_str(&format!(
            "| L/λ projected WIP time (days) | {} |\n",
            fmt_opt(self.metrics.projected_wip_time_days)
        ));
        out.push_str(&format!(
            "| Completed objects in {}d window | {} |\n\n",
            self.metrics.observation_window_days, self.metrics.completed_objects_in_window
        ));

        out.push_str("## Closure frontier\n\n");
        out.push_str("| Rank | Score | Action | Repository | WIP | Reason | Authority |\n");
        out.push_str("| ---: | ---: | --- | --- | --- | --- | --- |\n");
        for (idx, intent) in self.closure_frontier.iter().enumerate() {
            out.push_str(&format!(
                "| {} | {:.3} | {:?} | {} | {} | {} | {} |\n",
                idx + 1,
                intent.priority_score,
                intent.action,
                escape_md(&intent.repository),
                escape_md(&intent.wip_id),
                escape_md(&intent.reason),
                intent.authority
            ));
        }

        out.push_str("\n## Admitted WIP\n\n");
        out.push_str("| Kind | Standing | Repository | Age d | Object | Blockers |\n");
        out.push_str("| --- | --- | --- | ---: | --- | --- |\n");
        for wip in &self.wip {
            out.push_str(&format!(
                "| {:?} | {:?} | {} | {:.2} | {} | {} |\n",
                wip.kind,
                wip.standing,
                escape_md(&wip.repository),
                wip.age_days,
                escape_md(&wip.title),
                escape_md(&wip.blockers.join(", "))
            ));
        }

        if !self.notes.is_empty() {
            out.push_str("\n## Evidence boundary\n\n");
            for note in &self.notes {
                out.push_str(&format!("- {}\n", note));
            }
        }
        out
    }

    pub fn to_ocel_value(&self) -> Value {
        let repositories: BTreeSet<String> =
            self.wip.iter().map(|item| item.repository.clone()).collect();

        let mut objects = Vec::new();
        for repository in repositories {
            objects.push(json!({
                "id": format!("repo:{repository}"),
                "type": "Repository",
                "attributes": [
                    {"name": "full_name", "time": self.observed_at, "value": repository}
                ],
                "relationships": []
            }));
        }
        for item in &self.wip {
            objects.push(json!({
                "id": item.id,
                "type": "Wip",
                "attributes": [
                    {"name": "kind", "time": self.observed_at, "value": format!("{:?}", item.kind).to_uppercase()},
                    {"name": "standing", "time": self.observed_at, "value": format!("{:?}", item.standing).to_uppercase()},
                    {"name": "age_days", "time": self.observed_at, "value": item.age_days}
                ],
                "relationships": [
                    {"objectId": format!("repo:{}", item.repository), "qualifier": "belongs_to"}
                ]
            }));
        }

        let mut events = Vec::new();
        for item in &self.wip {
            events.push(json!({
                "id": format!("observed:{}", item.id),
                "type": "WipObserved",
                "time": self.observed_at,
                "attributes": [
                    {"name": "origin", "value": item.origin},
                    {"name": "standing", "value": format!("{:?}", item.standing).to_uppercase()}
                ],
                "relationships": [
                    {"objectId": item.id, "qualifier": "observed_wip"},
                    {"objectId": format!("repo:{}", item.repository), "qualifier": "repository"}
                ]
            }));
        }
        for intent in &self.closure_frontier {
            events.push(json!({
                "id": intent.id,
                "type": "ClosureIntentConstructed",
                "time": self.observed_at,
                "attributes": [
                    {"name": "action", "value": format!("{:?}", intent.action).to_uppercase()},
                    {"name": "priority_score", "value": intent.priority_score},
                    {"name": "authority", "value": intent.authority},
                    {"name": "receipt_required", "value": intent.receipt_required}
                ],
                "relationships": [
                    {"objectId": intent.wip_id, "qualifier": "targets"}
                ]
            }));
        }

        json!({
            "schema": "OCEL 2.0",
            "producer": "anti-llm-cheat-lsp/wip",
            "observation": {
                "report_schema": WIP_REPORT_SCHEMA,
                "observed_at": self.observed_at,
                "status": format!("{:?}", self.status).to_uppercase()
            },
            "objectTypes": [
                {"name": "Repository", "attributes": [{"name": "full_name", "type": "string"}]},
                {"name": "Wip", "attributes": [
                    {"name": "kind", "type": "string"},
                    {"name": "standing", "type": "string"},
                    {"name": "age_days", "type": "float"}
                ]}
            ],
            "eventTypes": [
                {"name": "WipObserved", "attributes": [
                    {"name": "origin", "type": "string"},
                    {"name": "standing", "type": "string"}
                ]},
                {"name": "ClosureIntentConstructed", "attributes": [
                    {"name": "action", "type": "string"},
                    {"name": "priority_score", "type": "float"},
                    {"name": "authority", "type": "string"},
                    {"name": "receipt_required", "type": "boolean"}
                ]}
            ],
            "objects": objects,
            "events": events
        })
    }
}

fn fmt_opt(v: Option<f64>) -> String {
    match v {
        Some(n) => format!("{n:.3}"),
        None => "UNKNOWN".to_string(),
    }
}

fn escape_md(input: &str) -> String {
    input.replace('|', "\\|").replace('\n', " ")
}
