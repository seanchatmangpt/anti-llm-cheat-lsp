use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // Route log treated as route execution
        if o.construct == "Routing to PackPlan"
            || o.context.contains("Routing to PackPlan -> Staging")
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-ROUTE-001".to_string(),
                category: "route".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Route log treated as route execution. A print or log of the route does not prove execution.".to_string(),
                forbidden_implication: "Log(RouteIntent) => RouteExecution".to_string(),
                blocking: true,
                required_correction: "Collect concrete evidence at each step of the route (CodeAction, clap admission, PackPlan, Staging, MutationGate, Receipt).".to_string(),
                required_next_proof: "Require active receipt matching the checkpoint.".to_string(),
            });
        }

        // Static scan substituted for route proof
        if (o.message.contains("static scan") && o.message.contains("route proof"))
            || o.construct == "static scan as route proof"
        {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-ROUTE-008".to_string(),
                category: "route".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "Static scan substituted for route proof. Lack of bad strings does not prove mutation was safely routed.".to_string(),
                forbidden_implication: "¬KnownBadPath => AllMutation lawfully routed".to_string(),
                blocking: true,
                required_correction: "Use dynamic mutation gate check instead of static text scan checks.".to_string(),
                required_next_proof: "Prove MutationGate denial handles unadmitted paths.".to_string(),
            });
        }
    }

    diags
}
