pub fn generate_implications_markdown() -> String {
    let mut out = String::new();
    out.push_str("# Forbidden Implications Reference\n\n");
    out.push_str("Each diagnostic category enforces the prevention of specific invalid logical leaps typical of LLM-generated solutions:\n\n");

    let implications = [
        ("ANTI-LLM-SURFACE-001", "Pass(plain LSP) ⇒ Pass(LSP 3.18)", "Assuming basic compatibility proves LSP 3.18 capability conformance."),
        ("ANTI-LLM-SURFACE-003", "Pack observes surface ⇒ runtime uses surface", "Assuming static scanning proves runtime capability utilization."),
        ("ANTI-LLM-SURFACE-005", "Basic LSP transcript ⇒ LSP 3.18", "LSP 3.18 features require explicit capabilities negotiated in initialize."),
        ("ANTI-LLM-AUTH-002", "Elegant abstraction ⇒ existing authority", "Inventing elegant abstractions (e.g. fake CLAP) instead of referencing concrete clap-noun-verb code."),
        ("ANTI-LLM-AUTH-004", "StringShape(command) ⇒ command admission", "Treating a raw command string match as a validated authority command admission."),
        ("ANTI-LLM-RECEIPT-001", "TestStdout ⇒ Receipt", "Treating plain test output as a signed process receipt."),
        ("ANTI-LLM-RECEIPT-002", "LogMessage ⇒ Receipt", "Using plain logger trace messages as mutation receipts."),
        ("ANTI-LLM-RECEIPT-003", "Claimed proof ⇒ receipt exists", "Claiming code/ontological safety without producing a physical receipt file."),
        ("ANTI-LLM-ROUTE-001", "Log(RouteIntent) ⇒ RouteExecution", "Treating a print statement logging a route as route execution evidence."),
        ("ANTI-LLM-ROUTE-008", "¬KnownBadPath ⇒ AllMutation lawfully routed", "Substituting static file filters for actual dynamic route gates."),
        ("ANTI-LLM-MUT-001", "LSP observation ⇒ mutation authority", "Allowing LSP servers to directly write files instead of returning template intents."),
        ("ANTI-LLM-MUT-002", "WorkspaceEdit ⇒ admitted receipt mutation", "Directly binding WorkspaceEdit edits to final cryptographic receipts."),
        ("ANTI-LLM-TEST-001", "String assertion ⇒ authority proof", "Testing string outputs instead of actual state boundaries."),
        ("ANTI-LLM-TEST-003", "Positive case passes ⇒ law holds", "Claiming safety without verifying negative controls."),
        ("ANTI-LLM-STRANGE-001", "Debug scaffold ⇒ law diagnostic", "Deploying debug diagnostics into production environments."),
        ("ANTI-LLM-STRANGE-002", "Raw content dump ⇒ useful diagnostic", "Dumping complete file content inside warning diagnostics."),
        ("ANTI-LLM-STRANGE-003", "Raw path dump ⇒ law diagnostic", "Dumping absolute file paths in diagnostics, exposing environment detail."),
        ("ANTI-LLM-STRANGE-007", "SubstringMatch ⇒ Authority", "Using substring checks as the ultimate standard of validation."),
        ("ANTI-LLM-VERSION-001", "Template default ⇒ release law", "Deploying with standard v1.0.0 placeholder versions."),
        ("ANTI-LLM-CLAIM-004", "StatusWord(ADMITTED) ⇒ admitted", "Claiming 'fully admitted' or victory when active failsets remain open."),
        ("ANTI-LLM-LSP318-COMB-001", "ChangelogCoverage(15 rows) \u{21d2} SpecCoverage(LSP 3.18)", "Treating 15-row changelog matrix as full LSP 3.18 combinatorial coverage."),
    ];

    out.push_str("| Diagnostic Code | Forbidden Implication | Description |\n");
    out.push_str("| --- | --- | --- |\n");
    for &(code, imp, desc) in &implications {
        out.push_str(&format!("| {} | `{}` | {} |\n", code, imp, desc));
    }

    out
}
