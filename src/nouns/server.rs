use clap_noun_verb::Result;
use clap_noun_verb_macros::verb;

/// Start the LSP server over stdio
#[verb]
pub fn serve(stdio: bool) -> Result<()> {
    if stdio {
        tokio::runtime::Handle::current().block_on(async {
            let stdin = tokio::io::stdin();
            let stdout = tokio::io::stdout();
            let (service, socket) = lsp_max::LspService::new(anti_llm_cheat_lsp::server::AntiLlmServer::new);
            let _ = lsp_max::Server::new(stdin, stdout, socket).serve(service).await;
        });
    } else {
        eprintln!("Error: --stdio flag is required for LSP serve");
        std::process::exit(1);
    }
    Ok(())
}

/// Run a raw scan on the workspace directory
#[verb]
pub fn scan(dir: String) -> Result<()> {
    // Determine the target directory (defaulting to current directory if not provided)
    let target_dir = if dir.is_empty() { ".".to_string() } else { dir };
    
    let _ = anti_llm_cheat_lsp::ocel::write_ocel_outputs(&target_dir);
    let obs = anti_llm_cheat_lsp::engine::scan_directory(&target_dir);
    let mut diags = anti_llm_cheat_lsp::engine::evaluate_diagnostics(&obs);
    diags.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line.cmp(&b.line)));
    
    println!("--- Anti-LLM Admissibility Scan Findings ---");
    println!("Observations: {}", obs.len());
    println!("Diagnostics emitted: {}", diags.len());
    for d in diags {
        println!("  - [{}] {}:{}: {}", d.code, d.file_path, d.line, d.message);
    }
    Ok(())
}
