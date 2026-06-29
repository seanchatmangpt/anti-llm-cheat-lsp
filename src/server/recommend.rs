//! Recommendation surface: project real cheat detections into LSP language
//! features. The engine already computes `required_correction` and
//! `required_next_proof` per detection (`diagnostics::AntiLlmDiagnostic`); this
//! module routes those recommendations through hover, code actions, code
//! lenses, document symbols and navigation so every wired capability reports
//! cheats to developers instead of returning a hollow stub.
//!
//! The surface stays read-only by law: it emits intents (commands, hovers,
//! symbols, locations) and never mutates a file. No file URIs are constructed
//! here — navigation reuses the caller's document URI — so the module carries
//! no path-encoding assumptions.

use lsp_max::lsp_types::*;

use crate::diagnostics::AntiLlmDiagnostic;

/// Zero-based LSP line for a detection (engine lines are 1-based).
fn line0(d: &AntiLlmDiagnostic) -> u32 {
    d.line.saturating_sub(1) as u32
}

/// Bounded severity word — never victory language.
fn severity_word(d: &AntiLlmDiagnostic) -> &'static str {
    if d.blocking {
        "BLOCKING"
    } else {
        "WARNING"
    }
}

/// Truncate a recommendation to a single, bounded line for inline display.
fn brief(s: &str) -> String {
    let first = s.lines().next().unwrap_or("");
    let mut out: String = first.chars().take(80).collect();
    if first.chars().count() > 80 || s.lines().nth(1).is_some() {
        out.push('…');
    }
    out
}

/// Hover: explain every detection on the hovered line, including the engine's
/// recommended correction and the next proof obligation.
pub fn hover(diags: &[AntiLlmDiagnostic], pos: Position) -> Option<Hover> {
    let hits: Vec<&AntiLlmDiagnostic> = diags.iter().filter(|d| line0(d) == pos.line).collect();
    if hits.is_empty() {
        return None;
    }
    let mut v = String::new();
    for d in hits {
        v.push_str(&format!("### {} — {}\n\n{}\n\n", d.code, severity_word(d), d.message));
        if !d.forbidden_implication.is_empty() {
            v.push_str(&format!("- **Forbidden implication:** {}\n", d.forbidden_implication));
        }
        if !d.required_correction.is_empty() {
            v.push_str(&format!("- **Recommended correction:** {}\n", d.required_correction));
        }
        if !d.required_next_proof.is_empty() {
            v.push_str(&format!("- **Required next proof:** {}\n", d.required_next_proof));
        }
        v.push('\n');
    }
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: v }),
        range: None,
    })
}

/// Document outline: each detection as a navigable symbol so a developer can
/// jump between cheats in the current file.
pub fn document_symbols(diags: &[AntiLlmDiagnostic]) -> Vec<DocumentSymbol> {
    diags
        .iter()
        .map(|d| {
            let r = Range::new(Position::new(line0(d), 0), Position::new(line0(d), 80));
            #[allow(deprecated)]
            DocumentSymbol {
                name: d.code.clone(),
                detail: Some(brief(&d.message)),
                kind: SymbolKind::EVENT,
                tags: None,
                deprecated: None,
                range: r,
                selection_range: r,
                children: None,
            }
        })
        .collect()
}

/// Code lenses: a file-level summary lens plus a per-detection lens carrying
/// the rule code and its recommended correction as command arguments.
pub fn code_lenses(diags: &[AntiLlmDiagnostic]) -> Vec<CodeLens> {
    let mut out = Vec::with_capacity(diags.len() + 1);
    let blocking = diags.iter().filter(|d| d.blocking).count();
    out.push(CodeLens {
        range: Range::new(Position::new(0, 0), Position::new(0, 1)),
        command: Some(Command {
            title: format!(
                "anti-llm: {} detection(s), {} blocking — open failset",
                diags.len(),
                blocking
            ),
            command: "anti-llm.openFailset".to_string(),
            arguments: None,
        }),
        data: None,
    });
    for d in diags {
        out.push(CodeLens {
            range: Range::new(Position::new(line0(d), 0), Position::new(line0(d), 1)),
            command: Some(Command {
                title: format!("{}: {}", d.code, severity_word(d)),
                command: "anti-llm.check".to_string(),
                arguments: Some(vec![serde_json::json!({
                    "code": d.code,
                    "correction": d.required_correction,
                    "nextProof": d.required_next_proof,
                })]),
            }),
            data: None,
        });
    }
    out
}

/// Repair-plan code actions: one quickfix per detection that carries a
/// recommended correction. Read-only — each action emits a command/intent and
/// attaches the originating diagnostic; it never edits the file.
pub fn repair_actions(diags: &[AntiLlmDiagnostic]) -> Vec<CodeActionOrCommand> {
    diags
        .iter()
        .filter(|d| !d.required_correction.is_empty())
        .map(|d| {
            CodeActionOrCommand::CodeAction(CodeAction {
                title: format!("{}: {}", d.code, brief(&d.required_correction)),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![d.to_lsp()]),
                command: Some(Command {
                    title: "Open receipt ledger".to_string(),
                    command: "anti-llm.openReceiptLedger".to_string(),
                    arguments: None,
                }),
                ..Default::default()
            })
        })
        .collect()
}

/// Same-file occurrences of the detection(s) under the cursor, keyed by rule
/// code. Navigation reuses the caller's document URI, so no path encoding is
/// assumed. Returns every line in this document sharing a rule code with the
/// cursor position.
pub fn same_file_locations(diags: &[AntiLlmDiagnostic], uri: &Uri, pos: Position) -> Vec<Location> {
    let codes: Vec<&str> =
        diags.iter().filter(|d| line0(d) == pos.line).map(|d| d.code.as_str()).collect();
    if codes.is_empty() {
        return Vec::new();
    }
    diags
        .iter()
        .filter(|d| codes.contains(&d.code.as_str()))
        .map(|d| Location {
            uri: uri.clone(),
            range: Range::new(Position::new(line0(d), 0), Position::new(line0(d), 10)),
        })
        .collect()
}
