use anti_llm_cheat_lsp::{diagnostics::AntiLlmDiagnostic, engine};

#[test]
fn test_victory_language_detected() {
    let content = "We have solved all problems and are fully guaranteed to work. Victory is ours!";
    let obs = engine::scan_file("test.md");
    // wait, scan_file uses filepath...
}
