//! Matrix-driven ServerCapabilities generator.
//!
//! This module derives ServerCapabilities from lsp318_coverage::surface_table,
//! ensuring advertised capabilities and handler wiring stay synchronized.
//! A capability is enabled only when its corresponding method has a Wired handler.

use lsp_max::lsp_types::*;

use crate::rules::lsp318_coverage::{full_surface, HandlerState};

// Capabilities are built from the coverage matrix; the `Refuses` variant is
// included when a capability must be advertised for the refusal path to be
// reachable by the client (e.g. rename must be declared so prepareRename fires).

/// Build ServerCapabilities by scanning the coverage matrix.
/// Only methods with Wired handlers get advertised capabilities.
pub fn build_capabilities() -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    let surface = full_surface();

    for method in surface {
        // Wired handlers get full capabilities; Refuses handlers get minimal
        // declarations so the refusal path is reachable by the client.
        let is_wired = method.handler == HandlerState::Wired;
        let is_refuses = method.handler == HandlerState::Refuses;
        if !is_wired && !is_refuses {
            continue;
        }

        match method.method {
            // ── Text document synchronization ─────────────────────────────────
            "textDocument/didOpen"
            | "textDocument/didChange"
            | "textDocument/didSave"
            | "textDocument/didClose"
            | "textDocument/willSave"
            | "textDocument/willSaveWaitUntil"
                if caps.text_document_sync.is_none() =>
            {
                caps.text_document_sync =
                    Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        will_save: Some(true),
                        will_save_wait_until: Some(true),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                    }));
            }

            // ── Navigation language features ──────────────────────────────────
            "textDocument/declaration" => {
                caps.declaration_provider = Some(DeclarationCapability::Simple(true));
            }
            "textDocument/definition" => {
                caps.definition_provider = Some(OneOf::Left(true));
            }
            "textDocument/typeDefinition" => {
                caps.type_definition_provider =
                    Some(TypeDefinitionProviderCapability::Simple(true));
            }
            "textDocument/implementation" => {
                caps.implementation_provider = Some(ImplementationProviderCapability::Simple(true));
            }
            "textDocument/references" => {
                caps.references_provider = Some(OneOf::Left(true));
            }
            "textDocument/documentHighlight" => {
                caps.document_highlight_provider = Some(OneOf::Left(true));
            }
            "textDocument/hover" => {
                caps.hover_provider = Some(HoverProviderCapability::Simple(true));
            }
            "textDocument/prepareRename" | "textDocument/rename"
                if caps.rename_provider.is_none() =>
            {
                // Rename is declared so prepareRename is reachable; both are
                // refused at the handler level (read-only law).
                caps.rename_provider = Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }));
            }
            "textDocument/selectionRange" => {
                caps.selection_range_provider =
                    Some(SelectionRangeProviderCapability::Simple(true));
            }
            "textDocument/linkedEditingRange" => {
                caps.linked_editing_range_provider =
                    Some(LinkedEditingRangeServerCapabilities::Simple(true));
            }
            "textDocument/moniker" => {
                caps.moniker_provider = Some(OneOf::Left(true));
            }

            // ── Completion / signature / lens / link / color / action ─────────
            "textDocument/completion" => {
                caps.completion_provider = Some(CompletionOptions {
                    resolve_provider: Some(true),
                    trigger_characters: Some(vec!["#".to_string()]),
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                    completion_item: None,
                });
            }
            "textDocument/signatureHelp" => {
                caps.signature_help_provider = Some(SignatureHelpOptions::default());
            }
            "textDocument/codeLens" => {
                caps.code_lens_provider = Some(CodeLensOptions { resolve_provider: Some(true) });
            }
            "codeLens/resolve" => {
                // Already handled by textDocument/codeLens
            }
            "textDocument/codeAction" if caps.code_action_provider.is_none() => {
                caps.code_action_provider = Some(CodeActionProviderCapability::Simple(true));
            }
            "codeAction/resolve" => {
                caps.code_action_provider =
                    Some(CodeActionProviderCapability::Options(CodeActionOptions {
                        code_action_kinds: None,
                        resolve_provider: Some(true),
                        work_done_progress_options: WorkDoneProgressOptions::default(),
                    }));
            }

            // ── Document link / color / on-type formatting ────────────────────
            "textDocument/documentLink" | "documentLink/resolve"
                if caps.document_link_provider.is_none() =>
            {
                caps.document_link_provider = Some(DocumentLinkOptions {
                    resolve_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                });
            }
            "textDocument/documentColor" | "textDocument/colorPresentation"
                if caps.color_provider.is_none() =>
            {
                caps.color_provider = Some(ColorProviderCapability::Simple(true));
            }
            "textDocument/onTypeFormatting"
                if caps.document_on_type_formatting_provider.is_none() =>
            {
                caps.document_on_type_formatting_provider = Some(DocumentOnTypeFormattingOptions {
                    first_trigger_character: ".".to_string(),
                    more_trigger_character: Some(vec!["(".to_string()]),
                });
            }

            // ── Formatting / folding / hints / inline / semantic / symbol ─────
            "textDocument/formatting" => {
                caps.document_formatting_provider = Some(OneOf::Left(true));
            }
            "textDocument/rangesFormatting" => {
                caps.document_range_formatting_provider = Some(OneOf::Left(true));
            }
            "textDocument/foldingRange" => {
                caps.folding_range_provider = Some(FoldingRangeProviderCapability::Simple(true));
            }
            "textDocument/inlineCompletion" => {
                caps.inline_completion_provider = Some(OneOf::Left(true));
            }
            "textDocument/inlayHint" | "inlayHint/resolve"
                if caps.inlay_hint_provider.is_none() =>
            {
                caps.inlay_hint_provider =
                    Some(OneOf::Right(InlayHintServerCapabilities::Options(InlayHintOptions {
                        resolve_provider: Some(true),
                        work_done_progress_options: WorkDoneProgressOptions::default(),
                    })));
            }
            "textDocument/inlineValue" => {
                caps.inline_value_provider = Some(OneOf::Left(true));
            }
            "textDocument/documentSymbol" => {
                caps.document_symbol_provider = Some(OneOf::Left(true));
            }

            // ── Semantic tokens (AST-derived, Path B) ─────────────────────────
            // The `full`, `full/delta` and `range` rows collapse onto one
            // capability block; build it once when the first of them is seen.
            "textDocument/semanticTokens/full"
            | "textDocument/semanticTokens/full/delta"
            | "textDocument/semanticTokens/range"
                if caps.semantic_tokens_provider.is_none() =>
            {
                caps.semantic_tokens_provider =
                    Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: crate::semantic::legend(),
                            full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                            range: Some(true),
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                        },
                    ));
            }

            // Type hierarchy is not supported in the sibling lsp-types-max ServerCapabilities.
            // "textDocument/prepareTypeHierarchy"
            // | "typeHierarchy/supertypes"
            // | "typeHierarchy/subtypes"
            //     if caps.type_hierarchy_provider.is_none() =>
            // {
            //     caps.type_hierarchy_provider = Some(OneOf::Left(true));
            // }

            // ── Call hierarchy ────────────────────────────────────────────────
            "textDocument/prepareCallHierarchy"
            | "callHierarchy/incomingCalls"
            | "callHierarchy/outgoingCalls"
                if caps.call_hierarchy_provider.is_none() =>
            {
                caps.call_hierarchy_provider = Some(CallHierarchyServerCapability::Simple(true));
            }

            // ── Pull diagnostics ──────────────────────────────────────────────
            "textDocument/diagnostic" | "workspace/diagnostic"
                if caps.diagnostic_provider.is_none() =>
            {
                caps.diagnostic_provider =
                    Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                        identifier: None,
                        inter_file_dependencies: true,
                        workspace_diagnostics: true,
                        work_done_progress_options: WorkDoneProgressOptions::default(),
                    }));
            }

            // ── Workspace features ────────────────────────────────────────────
            "workspace/symbol" if caps.workspace_symbol_provider.is_none() => {
                caps.workspace_symbol_provider = Some(OneOf::Left(true));
            }
            "workspaceSymbol/resolve" => {
                // Handler wired; capability already set by workspace/symbol arm.
                // Resolve support is declared via the handler being present.
            }
            "workspace/willCreateFiles"
            | "workspace/willRenameFiles"
            | "workspace/willDeleteFiles"
            | "workspace/didCreateFiles"
            | "workspace/didRenameFiles"
            | "workspace/didDeleteFiles"
            | "workspace/didChangeWorkspaceFolders"
            | "workspace/workspaceFolders"
                if caps.workspace.is_none() =>
            {
                caps.workspace = Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                        did_create: Some(FileOperationRegistrationOptions { filters: vec![] }),
                        will_create: Some(FileOperationRegistrationOptions { filters: vec![] }),
                        did_rename: Some(FileOperationRegistrationOptions { filters: vec![] }),
                        will_rename: Some(FileOperationRegistrationOptions { filters: vec![] }),
                        did_delete: Some(FileOperationRegistrationOptions { filters: vec![] }),
                        will_delete: Some(FileOperationRegistrationOptions { filters: vec![] }),
                    }),
                    text_document_content: None,
                });
            }
            "workspace/executeCommand" => {
                caps.execute_command_provider = Some(ExecuteCommandOptions {
                    commands: vec![
                        "anti-llm.validateReceiptChain".to_string(),
                        "anti-llm.exportOcel".to_string(),
                    ],
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                });
            }
            "workspace/textDocumentContent" => {
                // No explicit capability field; handled by method availability
            }

            // ── Notebook documents ────────────────────────────────────────────
            "notebookDocument/didOpen"
            | "notebookDocument/didChange"
            | "notebookDocument/didSave"
            | "notebookDocument/didClose"
                if caps.notebook_document_sync.is_none() =>
            {
                caps.notebook_document_sync = Some(OneOf::Left(NotebookDocumentSyncOptions {
                    notebook_selector: vec![NotebookSelector::ByNotebook {
                        notebook: Notebook::String("*".to_string()),
                        cells: None,
                    }],
                    save: Some(true),
                }));
            }

            // ── Server-to-client refreshes ────────────────────────────────────
            "workspace/codeLens/refresh"
            | "workspace/semanticTokens/refresh"
            | "workspace/foldingRange/refresh"
            | "workspace/inlayHint/refresh"
            | "workspace/inlineValue/refresh"
            | "workspace/diagnostic/refresh" => {
                // Server-side refreshes; no client capability field needed
            }

            // ── Window features ───────────────────────────────────────────────
            "window/logMessage"
            | "window/showMessage"
            | "window/showMessageRequest"
            | "window/showDocument"
            | "window/workDoneProgress/create"
            | "window/workDoneProgress/cancel" => {
                // Server-side notifications/requests; no explicit server capability field
            }

            // ── Base protocol / general ───────────────────────────────────────
            "$/progress"
            | "$/setTrace"
            | "$/logTrace"
            | "client/registerCapability"
            | "client/unregisterCapability"
            | "telemetry/event"
            | "workspace/configuration"
            | "workspace/workspaceFolders"
            | "workspace/didChangeConfiguration"
            | "workspace/didChangeWatchedFiles" => {
                // No server capability field; handled at the protocol level
            }

            _ => {}
        }
    }

    caps
}
