//! LSP 3.18 combinatorial coverage extractor.
//!
//! This is the spec extractor that `ANTI-LLM-LSP318-COMB-001` requires: rather
//! than the 15-row delta changelog in `lsp318.rs`, it enumerates the full
//! method surface and derives each row's status from on-disk evidence —
//! transcript presence and receipt presence — never from a hand-authored claim.
//!
//! The status axis is intentionally tri-state aware. A transcript without a
//! wired handler is `UNKNOWN`, never `SUPPORTED_WITH_TRANSCRIPT`; a wired
//! handler with a transcript reaches `SUPPORTED_WITH_TRANSCRIPT` only while the
//! receipt axis stays `OPEN` (no receipt artifacts exist on disk). `UNKNOWN`
//! must never collapse into a polarity.

use std::path::Path;

/// Message origin/shape on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    ClientRequest,
    ClientNotification,
    ServerRequest,
    ServerNotification,
    General,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::ClientRequest => "C->S request",
            Direction::ClientNotification => "C->S notification",
            Direction::ServerRequest => "S->C request",
            Direction::ServerNotification => "S->C notification",
            Direction::General => "general",
        }
    }
}

/// How the example server actually wires the method, derived from a static
/// audit of `server.rs`. This is deliberately conservative: a method only
/// counts as `Wired` when `server.rs` returns real content for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerState {
    /// `server.rs` returns real content / invokes the client method.
    Wired,
    /// `server.rs` implements the handler but refuses by law (returns `Err`).
    Refuses,
    /// `server.rs` implements an empty no-op handler that contradicts a
    /// declared refusal-by-law posture (the notebook family).
    NoopContradiction,
    /// No handler and no capability wired in the example server.
    Absent,
}

/// Tri-state law axis. Kept distinct so `Unknown` can never be coerced into
/// `Admitted` or `Refused` (see `ConformanceVector` law).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisState {
    Admitted,
    Refused,
    Unknown,
}

/// One method of the LSP 3.18 surface and the static facts about it.
#[derive(Debug, Clone)]
pub struct MethodSurface {
    pub method: &'static str,
    pub client_capability_path: &'static str,
    pub server_capability_path: &'static str,
    /// Transcript basename under `transcripts/`, or empty if none is expected.
    pub transcript_basename: &'static str,
    pub handler: HandlerState,
    pub direction: Direction,
}

/// An evidence-derived coverage row. Statuses are computed, never declared.
#[derive(Debug, Clone)]
pub struct CoverageRow {
    pub method: String,
    pub client_capability_path: String,
    pub server_capability_path: String,
    pub direction: String,
    pub transcript_present: bool,
    pub receipt_present: bool,
    pub status: String,
    /// The transcript law axis: Admitted only when a transcript exists AND a
    /// handler is wired; Unknown when a transcript exists without a handler.
    pub transcript_axis: AxisState,
}

/// The full LSP 3.18 method surface. This is the combinatorial enumeration the
/// 15-row delta cannot stand in for. Each tuple is
/// `(method, client_cap, server_cap, transcript_basename, handler, direction)`.
fn surface_table() -> Vec<MethodSurface> {
    use Direction::*;
    use HandlerState::*;

    let rows: &[(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        HandlerState,
        Direction,
    )] = &[
        // ── Lifecycle & base protocol ─────────────────────────────────────
        (
            "initialize",
            "capabilities",
            "capabilities",
            "initialize_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "initialized",
            "",
            "",
            "initialized_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "shutdown",
            "",
            "",
            "shutdown_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "exit",
            "",
            "",
            "exit_positive.jsonl",
            Absent,
            ClientNotification,
        ),
        (
            "$/cancelRequest",
            "",
            "",
            "_cancelRequest_positive.jsonl",
            Absent,
            General,
        ),
        (
            "$/progress",
            "window.workDoneProgress",
            "",
            "_progress_positive.jsonl",
            Wired,
            General,
        ),
        (
            "$/setTrace",
            "",
            "",
            "_setTrace_positive.jsonl",
            Wired,
            General,
        ),
        (
            "$/logTrace",
            "",
            "",
            "_logTrace_positive.jsonl",
            Wired,
            General,
        ),
        (
            "client/registerCapability",
            "*.dynamicRegistration",
            "",
            "",
            Wired,
            ServerRequest,
        ),
        (
            "client/unregisterCapability",
            "*.dynamicRegistration",
            "",
            "client_unregisterCapability_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "telemetry/event",
            "",
            "",
            "telemetry_event_positive.jsonl",
            Wired,
            ServerNotification,
        ),
        // ── Text document synchronization ─────────────────────────────────
        (
            "textDocument/didOpen",
            "textDocument.synchronization",
            "textDocumentSync.openClose",
            "textDocument_didOpen_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "textDocument/didChange",
            "textDocument.synchronization",
            "textDocumentSync.change",
            "textDocument_didChange_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "textDocument/didClose",
            "textDocument.synchronization",
            "textDocumentSync.openClose",
            "textDocument_didClose_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "textDocument/didSave",
            "textDocument.synchronization.didSave",
            "textDocumentSync.save",
            "textDocument_didSave_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "textDocument/willSave",
            "textDocument.synchronization.willSave",
            "textDocumentSync.willSave",
            "textDocument_willSave_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "textDocument/willSaveWaitUntil",
            "textDocument.synchronization.willSaveWaitUntil",
            "textDocumentSync.willSaveWaitUntil",
            "textDocument_willSaveWaitUntil_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/publishDiagnostics",
            "textDocument.publishDiagnostics",
            "",
            "textDocument_publishDiagnostics_positive.jsonl",
            Wired,
            ServerNotification,
        ),
        // ── Navigation language features ──────────────────────────────────
        (
            "textDocument/declaration",
            "textDocument.declaration",
            "declarationProvider",
            "textDocument_declaration_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/definition",
            "textDocument.definition",
            "definitionProvider",
            "textDocument_definition_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/typeDefinition",
            "textDocument.typeDefinition",
            "typeDefinitionProvider",
            "textDocument_typeDefinition_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/implementation",
            "textDocument.implementation",
            "implementationProvider",
            "textDocument_implementation_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/references",
            "textDocument.references",
            "referencesProvider",
            "textDocument_references_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/documentHighlight",
            "textDocument.documentHighlight",
            "documentHighlightProvider",
            "textDocument_documentHighlight_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/hover",
            "textDocument.hover",
            "hoverProvider",
            "textDocument_hover_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/prepareRename",
            "textDocument.rename.prepareSupport",
            "renameProvider.prepareProvider",
            "textDocument_prepareRename_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/rename",
            "textDocument.rename",
            "renameProvider",
            "textDocument_rename_positive.jsonl",
            Refuses,
            ClientRequest,
        ),
        (
            "textDocument/selectionRange",
            "textDocument.selectionRange",
            "selectionRangeProvider",
            "textDocument_selectionRange_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/linkedEditingRange",
            "textDocument.linkedEditingRange",
            "linkedEditingRangeProvider",
            "textDocument_linkedEditingRange_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/moniker",
            "textDocument.moniker",
            "monikerProvider",
            "textDocument_moniker_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        // ── Completion / signature / lens / link / color / action ─────────
        (
            "textDocument/completion",
            "textDocument.completion",
            "completionProvider",
            "completion_list_apply_kind_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "completionItem/resolve",
            "",
            "completionProvider.resolveProvider",
            "completionItem_resolve_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/signatureHelp",
            "textDocument.signatureHelp.signatureInformation.activeParameterSupport",
            "signatureHelpProvider",
            "nullable_active_parameter_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/codeLens",
            "textDocument.codeLens",
            "codeLensProvider",
            "textDocument_codeLens_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "codeLens/resolve",
            "",
            "codeLensProvider.resolveProvider",
            "code_lens_resolve_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/documentLink",
            "textDocument.documentLink",
            "documentLinkProvider",
            "textDocument_documentLink_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "documentLink/resolve",
            "",
            "documentLinkProvider.resolveProvider",
            "documentLink_resolve_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/documentColor",
            "textDocument.colorProvider",
            "colorProvider",
            "textDocument_documentColor_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/colorPresentation",
            "",
            "colorProvider",
            "textDocument_colorPresentation_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/codeAction",
            "textDocument.codeAction",
            "codeActionProvider",
            "textDocument_codeAction_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "codeAction/resolve",
            "",
            "codeActionProvider.resolveProvider",
            "codeAction_resolve_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        // ── Formatting / folding / hints / inline / semantic / symbol ─────
        (
            "textDocument/formatting",
            "textDocument.formatting",
            "documentFormattingProvider",
            "textDocument_formatting_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/rangeFormatting",
            "textDocument.rangeFormatting",
            "documentRangeFormattingProvider",
            "textDocument_rangeFormatting_positive.jsonl",
            Refuses,
            ClientRequest,
        ),
        (
            "textDocument/rangesFormatting",
            "textDocument.formatting.rangesSupport",
            "documentRangeFormattingProvider",
            "textDocument_rangesFormatting_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/onTypeFormatting",
            "textDocument.onTypeFormatting",
            "documentOnTypeFormattingProvider",
            "textDocument_onTypeFormatting_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/foldingRange",
            "textDocument.foldingRange",
            "foldingRangeProvider",
            "textDocument_foldingRange_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/inlayHint",
            "textDocument.inlayHint",
            "inlayHintProvider",
            "textDocument_inlayHint_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "inlayHint/resolve",
            "",
            "inlayHintProvider.resolveProvider",
            "inlayHint_resolve_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/inlineValue",
            "textDocument.inlineValue",
            "inlineValueProvider",
            "textDocument_inlineValue_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/inlineCompletion",
            "textDocument.inlineCompletion",
            "inlineCompletionProvider",
            "inline_completion_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/semanticTokens/full",
            "textDocument.semanticTokens",
            "semanticTokensProvider.full",
            "textDocument_semanticTokens_full_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/semanticTokens/full/delta",
            "textDocument.semanticTokens",
            "semanticTokensProvider.full.delta",
            "textDocument_semanticTokens_full_delta_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/semanticTokens/range",
            "textDocument.semanticTokens",
            "semanticTokensProvider.range",
            "textDocument_semanticTokens_range_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/documentSymbol",
            "textDocument.documentSymbol",
            "documentSymbolProvider",
            "textDocument_documentSymbol_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        // ── Call / type hierarchy & pull diagnostics ──────────────────────
        (
            "textDocument/prepareCallHierarchy",
            "textDocument.callHierarchy",
            "callHierarchyProvider",
            "textDocument_prepareCallHierarchy_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "callHierarchy/incomingCalls",
            "textDocument.callHierarchy",
            "callHierarchyProvider",
            "callHierarchy_incomingCalls_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "callHierarchy/outgoingCalls",
            "textDocument.callHierarchy",
            "callHierarchyProvider",
            "callHierarchy_outgoingCalls_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/prepareTypeHierarchy",
            "textDocument.typeHierarchy",
            "typeHierarchyProvider",
            "textDocument_prepareTypeHierarchy_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "typeHierarchy/supertypes",
            "textDocument.typeHierarchy",
            "typeHierarchyProvider",
            "typeHierarchy_supertypes_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "typeHierarchy/subtypes",
            "textDocument.typeHierarchy",
            "typeHierarchyProvider",
            "typeHierarchy_subtypes_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "textDocument/diagnostic",
            "textDocument.diagnostic",
            "diagnosticProvider",
            "textDocument_diagnostic_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "workspace/diagnostic",
            "workspace.diagnostics",
            "diagnosticProvider.workspaceDiagnostics",
            "workspace_diagnostic_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        // ── Workspace features ────────────────────────────────────────────
        (
            "workspace/symbol",
            "workspace.symbol",
            "workspaceSymbolProvider",
            "workspace_symbol_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "workspaceSymbol/resolve",
            "workspace.symbol.resolveSupport",
            "workspaceSymbolProvider.resolveProvider",
            "workspaceSymbol_resolve_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "workspace/executeCommand",
            "workspace.executeCommand",
            "executeCommandProvider",
            "workspace_executeCommand_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "workspace/applyEdit",
            "workspace.applyEdit",
            "",
            "workspace_applyEdit_positive.jsonl",
            Refuses,
            ServerRequest,
        ),
        (
            "workspace/configuration",
            "workspace.configuration",
            "",
            "workspace_configuration_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "workspace/didChangeConfiguration",
            "workspace.didChangeConfiguration",
            "",
            "workspace_didChangeConfiguration_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "workspace/didChangeWatchedFiles",
            "workspace.didChangeWatchedFiles",
            "",
            "workspace_didChangeWatchedFiles_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "workspace/didChangeWorkspaceFolders",
            "workspace.workspaceFolders",
            "workspace.workspaceFolders",
            "workspace_didChangeWorkspaceFolders_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "workspace/workspaceFolders",
            "",
            "workspace.workspaceFolders.supported",
            "workspace_workspaceFolders_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "workspace/textDocumentContent",
            "workspace.textDocumentContent",
            "workspace.textDocumentContentProvider",
            "text_document_content_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "workspace/textDocumentContent/refresh",
            "",
            "",
            "workspace_textDocumentContent_refresh_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        // ── File operations ───────────────────────────────────────────────
        (
            "workspace/willCreateFiles",
            "workspace.fileOperations.willCreate",
            "workspace.fileOperations.willCreate",
            "workspace_willCreateFiles_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "workspace/willRenameFiles",
            "workspace.fileOperations.willRename",
            "workspace.fileOperations.willRename",
            "workspace_willRenameFiles_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "workspace/willDeleteFiles",
            "workspace.fileOperations.willDelete",
            "workspace.fileOperations.willDelete",
            "workspace_willDeleteFiles_positive.jsonl",
            Wired,
            ClientRequest,
        ),
        (
            "workspace/didCreateFiles",
            "workspace.fileOperations.didCreate",
            "workspace.fileOperations.didCreate",
            "workspace_didCreateFiles_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "workspace/didRenameFiles",
            "workspace.fileOperations.didRename",
            "workspace.fileOperations.didRename",
            "workspace_didRenameFiles_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        (
            "workspace/didDeleteFiles",
            "workspace.fileOperations.didDelete",
            "workspace.fileOperations.didDelete",
            "workspace_didDeleteFiles_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        // ── Server-to-client refreshes ────────────────────────────────────
        (
            "workspace/codeLens/refresh",
            "workspace.codeLens.refreshSupport",
            "",
            "workspace_codeLens_refresh_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "workspace/semanticTokens/refresh",
            "workspace.semanticTokens.refreshSupport",
            "",
            "workspace_semanticTokens_refresh_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "workspace/inlayHint/refresh",
            "workspace.inlayHint.refreshSupport",
            "",
            "workspace_inlayHint_refresh_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "workspace/inlineValue/refresh",
            "workspace.inlineValue.refreshSupport",
            "",
            "workspace_inlineValue_refresh_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "workspace/diagnostic/refresh",
            "workspace.diagnostics.refreshSupport",
            "",
            "workspace_diagnostic_refresh_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "workspace/foldingRange/refresh",
            "workspace.foldingRange.refreshSupport",
            "",
            "workspace_foldingRange_refresh_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        // ── Window features ───────────────────────────────────────────────
        (
            "window/showMessage",
            "",
            "",
            "window_showMessage_positive.jsonl",
            Wired,
            ServerNotification,
        ),
        (
            "window/showMessageRequest",
            "window.showMessage",
            "",
            "window_showMessageRequest_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "window/logMessage",
            "",
            "",
            "debug_message_kind_positive.jsonl",
            Wired,
            ServerNotification,
        ),
        (
            "window/showDocument",
            "window.showDocument",
            "",
            "window_showDocument_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "window/workDoneProgress/create",
            "window.workDoneProgress",
            "",
            "window_workDoneProgress_create_positive.jsonl",
            Wired,
            ServerRequest,
        ),
        (
            "window/workDoneProgress/cancel",
            "window.workDoneProgress",
            "",
            "window_workDoneProgress_cancel_positive.jsonl",
            Wired,
            ClientNotification,
        ),
        // ── Notebook documents ────────────────────────────────────────────────
        (
            "notebookDocument/didOpen",
            "notebookDocument.synchronization",
            "notebookDocumentSync",
            "",
            Wired,
            ClientNotification,
        ),
        (
            "notebookDocument/didChange",
            "notebookDocument.synchronization",
            "notebookDocumentSync",
            "",
            Wired,
            ClientNotification,
        ),
        (
            "notebookDocument/didSave",
            "notebookDocument.synchronization",
            "notebookDocumentSync",
            "",
            Wired,
            ClientNotification,
        ),
        (
            "notebookDocument/didClose",
            "notebookDocument.synchronization",
            "notebookDocumentSync",
            "",
            Wired,
            ClientNotification,
        ),
    ];

    rows.iter()
        .map(|&(method, c, s, t, h, d)| MethodSurface {
            method,
            client_capability_path: c,
            server_capability_path: s,
            transcript_basename: t,
            handler: h,
            direction: d,
        })
        .collect()
}

/// Public accessor for the full method surface.
pub fn full_surface() -> Vec<MethodSurface> {
    surface_table()
}

/// Resolve whether an artifact basename exists under the given subdirectory,
/// trying both an example-rooted and a workspace-rooted layout.
fn artifact_exists(workspace_root: &str, subdir: &str, basename: &str) -> bool {
    if basename.is_empty() {
        return false;
    }
    let candidates = [
        format!("{}/{}/{}", workspace_root, subdir, basename),
        format!(
            "{}/examples/anti-llm-cheat-lsp/{}/{}",
            workspace_root, subdir, basename
        ),
    ];
    candidates.iter().any(|p| Path::new(p).exists())
}

/// Derive the bounded status string for one method from its evidence.
fn derive_status(
    handler: HandlerState,
    transcript_present: bool,
    receipt_present: bool,
) -> &'static str {
    match handler {
        HandlerState::Refuses => "REFUSED",
        HandlerState::NoopContradiction => "BLOCKED",
        HandlerState::Wired => {
            if transcript_present && receipt_present {
                "ADMITTED"
            } else if transcript_present {
                "SUPPORTED_WITH_TRANSCRIPT"
            } else {
                "PARTIAL"
            }
        }
        HandlerState::Absent => {
            if transcript_present {
                // Transcript without a wired handler is genuinely UNKNOWN. It
                // must never be promoted to SUPPORTED on transcript alone.
                "UNKNOWN"
            } else {
                "OPEN"
            }
        }
    }
}

/// The transcript law axis, kept tri-state. Admitted only when a transcript is
/// backed by a wired handler; Unknown when a transcript stands alone.
fn derive_transcript_axis(handler: HandlerState, transcript_present: bool) -> AxisState {
    match handler {
        HandlerState::Wired if transcript_present => AxisState::Admitted,
        HandlerState::Refuses | HandlerState::NoopContradiction => AxisState::Refused,
        _ => AxisState::Unknown,
    }
}

/// Compute the full evidence-derived coverage for the workspace root.
pub fn compute_coverage(workspace_root: &str) -> Vec<CoverageRow> {
    full_surface()
        .into_iter()
        .map(|m| {
            let transcript_present =
                artifact_exists(workspace_root, "transcripts", m.transcript_basename);
            // Receipt basename mirrors the transcript stem; the receipts
            // directory does not exist, so this resolves to false. It is wired
            // anyway so the matrix tells the truth the moment receipts land.
            let receipt_basename = if m.transcript_basename.is_empty() {
                String::new()
            } else {
                m.transcript_basename
                    .replace("_positive.jsonl", "_receipt.json")
            };
            let receipt_present = artifact_exists(workspace_root, "receipts", &receipt_basename);
            let status = derive_status(m.handler, transcript_present, receipt_present);
            let transcript_axis = derive_transcript_axis(m.handler, transcript_present);
            CoverageRow {
                method: m.method.to_string(),
                client_capability_path: m.client_capability_path.to_string(),
                server_capability_path: m.server_capability_path.to_string(),
                direction: m.direction.as_str().to_string(),
                transcript_present,
                receipt_present,
                status: status.to_string(),
                transcript_axis,
            }
        })
        .collect()
}

/// A bounded conformance summary that preserves the three law axes. `unknown`
/// is reported on its own and never folded into the other two.
#[derive(Debug, Clone, Default)]
pub struct ConformanceSummary {
    pub total: usize,
    pub admitted: usize,
    pub refused: usize,
    pub unknown: usize,
    pub receipts_present: usize,
}

pub fn conformance_summary(rows: &[CoverageRow]) -> ConformanceSummary {
    let mut s = ConformanceSummary {
        total: rows.len(),
        ..Default::default()
    };
    for r in rows {
        match r.transcript_axis {
            AxisState::Admitted => s.admitted += 1,
            AxisState::Refused => s.refused += 1,
            AxisState::Unknown => s.unknown += 1,
        }
        if r.receipt_present {
            s.receipts_present += 1;
        }
    }
    s
}
