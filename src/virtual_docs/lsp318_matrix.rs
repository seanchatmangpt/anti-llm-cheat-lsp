use crate::rules::lsp318::get_feature_matrix;

pub fn generate_matrix_markdown() -> String {
    let mut out = String::new();
    out.push_str("# LSP 3.18 Delta Changelog Coverage Matrix\n\n");
    out.push_str("NOTE: This matrix represents Delta Changelog Coverage only. It does not represent full LSP 3.18 combinatorial coverage. The full combinatorial surface, with statuses derived from on-disk evidence, is the extractor output at `anti-llm://lsp318-full-matrix`.\n\n");
    out.push_str("| Feature ID | Feature | Status | Client Capability Path | Server Capability Path | Method | Transcript Path | Receipt | Digest | Forbidden Implication Prevented |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for f in get_feature_matrix() {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            f.feature_id,
            f.feature,
            f.status,
            f.client_capability_path,
            f.server_capability_path,
            f.request_method,
            f.positive_transcript_path,
            f.receipt_path,
            f.digest,
            f.forbidden_substitution_prevented
        ));
    }
    out
}
