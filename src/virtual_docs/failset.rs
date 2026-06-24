use crate::diagnostics::AntiLlmDiagnostic;

pub fn generate_failset_markdown(diags: &[AntiLlmDiagnostic]) -> String {
    let mut out = String::new();
    out.push_str("# Active Admissibility Failset\n\n");
    if diags.is_empty() {
        out.push_str(
            "Status: **REPORTED_CLEAN_WITH_RAW_SCAN**\n\nNo blocking failset items detected.\n",
        );
    } else {
        out.push_str("Status: **FAILSET_NONEMPTY**\n\n");
        out.push_str(
            "| Code | Category | Path | Line | Message | Forbidden Implication | Blocking |\n",
        );
        out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
        for d in diags {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                d.code,
                d.category,
                d.file_path,
                d.line,
                d.message,
                d.forbidden_implication,
                d.blocking
            ));
        }
    }
    out
}
