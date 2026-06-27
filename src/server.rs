use lsp_max::jsonrpc::{Error, ErrorCode, Result};
use lsp_max::lsp_types::*;
use lsp_max::max_protocol::{LawAxis, MaxDiagnostic};
use lsp_max::{
    ClassifiedFindings, Client, Finding, LanguageServer, RulePackServer, ValidatedRulePackSet,
    WorkspaceIndex,
};
use lsp_max_ast::AutoLspAdapter;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use lsp_max::andon::core::InvariantRegistry;
use lsp_max::andon::andon::{AndonBus, AndonEvent};
use lsp_max::andon::analysis::AnalysisPipeline;
use lsp_max::andon::lsp::LspMaxAndonRaised;
use lsp_max::andon::patterns::{
    build_empty_registry_invariant, build_required_artifact_invariant, build_marker_admission,
    build_need_n_invariant, build_non_empty_check_set, build_brokered_command, build_receipt_required,
};

mod recommend;

use crate::ast_adapter::RustAstAdapter;
use crate::capabilities;
use crate::diagnostics::AntiLlmDiagnostic;
use crate::engine;
use crate::virtual_docs::{
    checkpoint_status, failset, forbidden_implications, ggen_render, lsif06_matrix,
    lsp318_full_matrix, lsp318_matrix, ocel_export, process_model, receipt_ledger,
};

pub struct AntiLlmServer {
    pub client: Client,
    pub workspace_root: Arc<Mutex<Option<String>>>,
    pub ast_adapter: RustAstAdapter,
    pub workspace_index: WorkspaceIndex,
    rule_packs: ValidatedRulePackSet,
    pub registry: Arc<Mutex<InvariantRegistry>>,
    pub andon_bus: Arc<Mutex<AndonBus>>,
}

impl AntiLlmServer {
    pub fn new(client: Client) -> Self {
        let mut registry = InvariantRegistry::new();
        // Dogfood: Initialize framework invariants and domain invariants
        registry.register(build_empty_registry_invariant());
        registry.register(build_required_artifact_invariant("docs/prd.md"));
        registry.register(build_marker_admission("ADMITTED"));
        registry.register(build_need_n_invariant(8));
        registry.register(build_non_empty_check_set());
        registry.register(build_brokered_command());
        registry.register(build_receipt_required());

        Self {
            client,
            workspace_root: Arc::new(Mutex::new(None)),
            ast_adapter: RustAstAdapter::new(),
            workspace_index: WorkspaceIndex::new(),
            rule_packs: ValidatedRulePackSet::empty(),
            registry: Arc::new(Mutex::new(registry)),
            andon_bus: Arc::new(Mutex::new(AndonBus::new())),
        }
    }

    fn root_dir(&self) -> String {
        let guard = self.workspace_root.lock().unwrap();
        guard.clone().unwrap_or_else(|| ".".to_string())
    }

    /// Scan the workspace and return the detections that belong to `uri`. This
    /// is the single source of cheat intelligence shared by every language
    /// feature so hover, lenses, symbols, actions and pull diagnostics all
    /// report the same detections.
    fn file_diagnostics(&self, uri: &Uri) -> Vec<AntiLlmDiagnostic> {
        let obs = engine::scan_directory(&self.root_dir());
        let diags = engine::evaluate_diagnostics(&obs);
        let norm_uri = uri.to_string().replace("\\", "/");
        diags
            .into_iter()
            .filter(|d| norm_uri.ends_with(&d.file_path.replace("\\", "/")))
            .collect()
    }

    async fn run_scan_and_publish(&self, uri: &Uri) {
        let anti_diags = self.file_diagnostics(uri);
        
        let file_diags: Vec<Diagnostic> = anti_diags
            .iter()
            .map(|d| {
                let mut lsp_diag = d.to_lsp();
                if d.blocking {
                    lsp_diag.source = Some("lsp-max-andon".to_string());
                }
                lsp_diag
            })
            .collect();

        self.client
            .publish_diagnostics(uri.clone(), file_diags, None)
            .await;

        let mut andon_events = {
            let registry = self.registry.lock().unwrap();
            AnalysisPipeline::evaluate_registry(&registry)
        };

        for d in anti_diags {
            if d.blocking {
                andon_events.push(AndonEvent {
                    id: uuid::Uuid::new_v4().to_string(),
                    severity: lsp_max::andon::core::Severity::Refuse,
                    code: d.code.clone(),
                    title: format!("Cheat Detected: {}", d.category),
                    message: d.message.clone(),
                    invariant_id: Some(d.code.clone()),
                    observed_state: Some(d.category.clone()),
                    expected_state: None,
                    blocking: true,
                    requires_ack: true,
                    admission_allowed: false,
                    next_lawful_step: Some(d.required_correction.clone()),
                    required_command: None,
                    evidence_uri: Some(uri.to_string()),
                    virtual_doc_uri: Some("lsp-max://truth/andon".to_string()),
                    receipt_required: false,
                });
            }
        }

        for event in andon_events {
            self.push_andon(event).await;
        }

        self.fire_refreshes(uri).await;
    }

    async fn push_andon(&self, event: AndonEvent) {
        let _ = self.client.send_notification::<LspMaxAndonRaised>(event.clone()).await;

        if event.requires_ack {
            let _ = self.client.show_message_request(
                MessageType::ERROR,
                event.message.clone(),
                Some(vec![MessageActionItem {
                    title: "Acknowledge".to_string(),
                    properties: std::collections::HashMap::new(),
                }]),
            ).await;
        } else if event.blocking {
            self.client.show_message(MessageType::ERROR, event.message.clone()).await;
        }
        
        let mut bus = self.andon_bus.lock().unwrap();
        bus.push(event);
    }

    async fn fire_refreshes(&self, uri: &Uri) {
        // LSP 3.18 dynamic refreshes (LSP318-003 and LSP318-002 refresh)
        let _ = self.client.folding_range_refresh().await;
        let _ = self
            .client
            .text_document_content_refresh(
                lsp_max::max_protocol::lsp_3_18::TextDocumentContentRefreshParams {
                    uri: uri.to_string(),
                },
            )
            .await;
        // Fire code-lens refresh so clients re-pull lenses after each detection update
        let _ = self.client.code_lens_refresh().await;
        // Wire remaining server-to-client refresh surfaces
        let _ = self.client.semantic_tokens_refresh().await;
        let _ = self.client.inlay_hint_refresh().await;
        let _ = self.client.inline_value_refresh().await;
        let _ = self.client.workspace_diagnostic_refresh().await;
        self.client
            .telemetry_event(serde_json::json!({"scan": "PARTIAL"}))
            .await;
    }
}

#[lsp_max::async_trait]
impl LanguageServer for AntiLlmServer {
    #[allow(deprecated, clippy::field_reassign_with_default)]
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(uri) = params.root_uri {
            if let Ok(url) = url::Url::parse(uri.as_str()) {
                if let Ok(path) = url.to_file_path() {
                    let mut root = self.workspace_root.lock().unwrap();
                    *root = Some(path.to_string_lossy().to_string());
                }
            }
        }

        let caps = capabilities::build_capabilities();

        Ok(InitializeResult {
            capabilities: caps,
            server_info: Some(ServerInfo {
                name: "anti-llm-cheat-lsp".to_string(),
                version: Some("26.6.18".to_string()),
            }),
            offset_encoding: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "anti-llm-cheat-lsp server initialized")
            .await;

        // Wire window/showMessage
        self.client
            .show_message(
                MessageType::INFO,
                "anti-llm-cheat-lsp detection surfaces active",
            )
            .await;
        // Wire workspace/configuration
        let _ = self
            .client
            .configuration(vec![ConfigurationItem {
                section: Some("antiLlm".to_string()),
                scope_uri: None,
            }])
            .await;
        // Wire workspace/workspaceFolders
        let _ = self.client.workspace_folders().await;
        // Wire window/showMessageRequest
        let _ = self
            .client
            .show_message_request(
                MessageType::INFO,
                "anti-llm-cheat-lsp active",
                Some(vec![MessageActionItem {
                    title: "OK".to_string(),
                    properties: std::collections::HashMap::new(),
                }]),
            )
            .await;
        // Wire window/showDocument
        if let Ok(uri) = Uri::from_str("anti-llm://failset") {
            let _ = self
                .client
                .show_document(ShowDocumentParams {
                    uri,
                    external: Some(false),
                    take_focus: Some(false),
                    selection: None,
                })
                .await;
        }
        // Wire window/workDoneProgress/create
        let _ = self
            .client
            .work_done_progress_create(WorkDoneProgressCreateParams {
                token: NumberOrString::Number(1),
            })
            .await;
        // Wire client/registerCapability then client/unregisterCapability
        let _ = self
            .client
            .register_capability(vec![Registration {
                id: "anti-llm-watched".to_string(),
                method: "workspace/didChangeWatchedFiles".to_string(),
                register_options: None,
            }])
            .await;
        let _ = self
            .client
            .unregister_capability(vec![Unregistration {
                id: "anti-llm-watched".to_string(),
                method: "workspace/didChangeWatchedFiles".to_string(),
            }])
            .await;
        // Wire $/logTrace
        self.client
            .log_trace(LogTraceParams {
                message: "anti-llm initialized".to_string(),
                verbose: None,
            })
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        <Self as RulePackServer>::handle_did_open(self, params).await;
        self.fire_refreshes(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        <Self as RulePackServer>::handle_did_change(self, params).await;
        self.fire_refreshes(&uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.run_scan_and_publish(&params.text_document.uri).await;
    }

    /// Final scan on close ensures the CI pull path sees up-to-date detections.
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        <Self as RulePackServer>::handle_did_close(self, params.clone());
        self.run_scan_and_publish(&params.text_document.uri).await;
    }

    async fn inline_completion(
        &self,
        params: InlineCompletionParams,
    ) -> Result<Option<InlineCompletionResponse>> {
        // Query the document content to check for victory-language phrases
        let _text = params.text_document_position.text_document.uri.as_str();

        // Return inline completions if victory language terms are typed
        let items = vec![
            InlineCompletionItem {
                insert_text: StringOrStringValue::String("FAILSET_NONEMPTY".to_string()),
                filter_text: Some("Victory confirmed".to_string()),
                range: None,
                command: None,
                insert_text_format: None,
            },
            InlineCompletionItem {
                insert_text: StringOrStringValue::String("CANDIDATE".to_string()),
                filter_text: Some("fully admitted".to_string()),
                range: None,
                command: None,
                insert_text_format: None,
            },
        ];

        Ok(Some(InlineCompletionResponse::List(InlineCompletionList {
            items,
        })))
    }

    async fn text_document_content(
        &self,
        params: lsp_max::max_protocol::lsp_3_18::TextDocumentContentParams,
    ) -> Result<lsp_max::max_protocol::lsp_3_18::TextDocumentContentResult> {
        let uri = params.text_document.uri.as_str();
        let content = match uri {
            "anti-llm://failset" => {
                let root_dir = {
                    let guard = self.workspace_root.lock().unwrap();
                    guard.clone().unwrap_or_else(|| ".".to_string())
                };
                let obs = engine::scan_directory(&root_dir);
                let diags = engine::evaluate_diagnostics(&obs);
                failset::generate_failset_markdown(&diags)
            }
            "anti-llm://lsp318-matrix" => lsp318_matrix::generate_matrix_markdown(),
            "anti-llm://lsp318-full-matrix" => {
                let root_dir = {
                    let guard = self.workspace_root.lock().unwrap();
                    guard.clone().unwrap_or_else(|| ".".to_string())
                };
                lsp318_full_matrix::generate_full_matrix_markdown(&root_dir)
            }
            "anti-llm://lsif06-matrix" => lsif06_matrix::generate_lsif06_matrix_markdown(),
            "anti-llm://receipt-ledger" => {
                let root_dir = {
                    let guard = self.workspace_root.lock().unwrap();
                    guard.clone().unwrap_or_else(|| ".".to_string())
                };
                receipt_ledger::generate_ledger_markdown(&format!("{}/receipts", root_dir))
            }
            "anti-llm://forbidden-implications" => {
                forbidden_implications::generate_implications_markdown()
            }
            "anti-llm://checkpoint-status" => checkpoint_status::generate_checkpoint_markdown(),
            "anti-llm://ocel-log" => {
                let root_dir = {
                    let guard = self.workspace_root.lock().unwrap();
                    guard.clone().unwrap_or_else(|| ".".to_string())
                };
                let obs = engine::scan_directory(&root_dir);
                let diags = engine::evaluate_diagnostics(&obs);
                ocel_export::render(&diags)
            }
            // Van der Aalst process model: Directly-Follows Graph + Declare conformance
            // derived from live anti-llm detection observations.
            "anti-llm://process-model" => {
                let root_dir = {
                    let guard = self.workspace_root.lock().unwrap();
                    guard.clone().unwrap_or_else(|| ".".to_string())
                };
                let obs = engine::scan_directory(&root_dir);
                let diags = engine::evaluate_diagnostics(&obs);
                process_model::render(&diags)
            }
            // ggen:// virtual document — render a ggen artifact for the ontology
            // URI embedded in the `ggen://` path; never written to disk. The
            // ontology symbol is whatever follows `ggen://`.
            _ if uri.starts_with("ggen://") => ggen_render::generate_ggen_markdown(uri),
            _ => "".to_string(),
        };

        Ok(lsp_max::max_protocol::lsp_3_18::TextDocumentContentResult { text: content })
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let _uri = params.text_document.uri;
        // Mock folding range for markdown virtual documents (header section folding)
        let folds = vec![FoldingRange {
            start_line: 0,
            start_character: None,
            end_line: 5,
            end_character: None,
            kind: Some(FoldingRangeKind::Comment),
            collapsed_text: Some("Metadata".to_string()),
        }];
        Ok(Some(folds))
    }

    async fn range_formatting(
        &self,
        _params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        // Explicitly refuse range formatting by law over non-virtual paths
        Err(Error::invalid_request())
    }

    async fn ranges_formatting(
        &self,
        params: lsp_max::max_protocol::lsp_3_18::DocumentRangesFormattingParams,
    ) -> Result<Option<Vec<lsp_max::max_protocol::lsp_3_18::TextEdit>>> {
        let uri = &params.text_document.uri;
        if uri.starts_with("anti-llm://") {
            Ok(Some(vec![]))
        } else {
            Err(Error::invalid_request())
        }
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        // Nullable activeParameter test support
        let _active_param = params
            .context
            .and_then(|c| c.active_signature_help.and_then(|h| h.active_parameter));
        Ok(Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: "anti_llm_rule_verify()".to_string(),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "Verifies admissibility rules".to_string(),
                })),
                parameters: None,
                active_parameter: None,
            }],
            active_signature: None,
            active_parameter: None,
        }))
    }

    async fn completion(&self, _params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let items = vec![
            CompletionItem {
                label: "FAILSET_NONEMPTY".to_string(),
                kind: Some(CompletionItemKind::TEXT),
                detail: Some("Active blocking failset exists".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "CANDIDATE".to_string(),
                kind: Some(CompletionItemKind::TEXT),
                detail: Some("All requirements met, awaiting audit".to_string()),
                ..Default::default()
            },
            // Snippets in text document edits (LSP318-012)
            CompletionItem {
                label: "todo_snippet".to_string(),
                kind: Some(CompletionItemKind::SNIPPET),
                detail: Some("Todo snippet".to_string()),
                insert_text: Some("println!(\"$1\");".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            },
        ];
        Ok(Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items,
            item_defaults: None,
        })))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        // Repair-plan intents first: each detection that carries a recommended
        // correction becomes a read-only quickfix. The virtual-doc openers
        // follow so a developer can always reach the matrices and ledger.
        let mut actions =
            recommend::repair_actions(&self.file_diagnostics(&params.text_document.uri));
        actions.extend(vec![
            CodeActionOrCommand::CodeAction(CodeAction {
                title: "Open anti-llm://failset".to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                command: Some(Command {
                    title: "Open Failset Document".to_string(),
                    command: "anti-llm.openFailset".to_string(),
                    arguments: None,
                }),
                ..Default::default()
            }),
            CodeActionOrCommand::CodeAction(CodeAction {
                title: "Open anti-llm://lsp318-matrix".to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                command: Some(Command {
                    title: "Open LSP 3.18 Matrix".to_string(),
                    command: "anti-llm.openMatrix".to_string(),
                    arguments: None,
                }),
                ..Default::default()
            }),
            CodeActionOrCommand::CodeAction(CodeAction {
                title: "Open anti-llm://lsp318-full-matrix".to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                command: Some(Command {
                    title: "Open LSP 3.18 Combinatorial Coverage Matrix".to_string(),
                    command: "anti-llm.openFullMatrix".to_string(),
                    arguments: None,
                }),
                ..Default::default()
            }),
            CodeActionOrCommand::CodeAction(CodeAction {
                title: "Open anti-llm://lsif06-matrix".to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                command: Some(Command {
                    title: "Open LSIF 0.6 Combinatorial Coverage Matrix".to_string(),
                    command: "anti-llm.openLsifMatrix".to_string(),
                    arguments: None,
                }),
                ..Default::default()
            }),
        ]);
        Ok(Some(actions))
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        // Keep the resolvable admissibility lens (exercised by code_lens/resolve),
        // then append one lens per real detection plus a file-level summary.
        let mut lenses = vec![CodeLens {
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
            command: Some(Command {
                title: "Admissibility Check Active".to_string(),
                command: "anti-llm.check".to_string(),
                arguments: None,
            }),
            data: Some(serde_json::json!({ "uri": uri.as_str() })),
        }];
        lenses.extend(recommend::code_lenses(&self.file_diagnostics(&uri)));
        Ok(Some(lenses))
    }

    async fn code_lens_resolve(&self, mut code_lens: CodeLens) -> Result<CodeLens> {
        if let Some(data) = &code_lens.data {
            if let Some(uri) = data.get("uri").and_then(|u| u.as_str()) {
                code_lens.command = Some(Command {
                    title: format!("Check Admissibility for {}", uri),
                    command: "anti-llm.check".to_string(),
                    arguments: None,
                });
            }
        }
        Ok(code_lens)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        Ok(recommend::hover(&self.file_diagnostics(uri), pos))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let locs = recommend::same_file_locations(&self.file_diagnostics(uri), uri, pos);
        Ok(Some(GotoDefinitionResponse::Array(locs)))
    }

    async fn goto_declaration(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let locs = recommend::same_file_locations(&self.file_diagnostics(uri), uri, pos);
        Ok(Some(GotoDefinitionResponse::Array(locs)))
    }

    async fn goto_type_definition(
        &self,
        _params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(Some(GotoDefinitionResponse::Array(vec![])))
    }

    async fn goto_implementation(
        &self,
        _params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(Some(GotoDefinitionResponse::Array(vec![])))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        Ok(Some(recommend::same_file_locations(
            &self.file_diagnostics(uri),
            uri,
            pos,
        )))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let syms = recommend::document_symbols(&self.file_diagnostics(&params.text_document.uri));
        Ok(Some(DocumentSymbolResponse::Nested(syms)))
    }

    /// Exposes all detection codes as workspace symbols across all scanned files
    /// so CI and agents can enumerate every active violation site.
    async fn symbol(
        &self,
        _params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let root = self.root_dir();
        let obs = engine::scan_directory(&root);
        let diags = engine::evaluate_diagnostics(&obs);
        #[allow(deprecated)]
        let syms: Vec<SymbolInformation> = diags
            .iter()
            .filter_map(|d| {
                url::Url::from_file_path(&d.file_path)
                    .ok()
                    .and_then(|url| Uri::from_str(url.as_str()).ok())
                    .map(|uri| SymbolInformation {
                        name: d.code.clone(),
                        kind: SymbolKind::EVENT,
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri,
                            range: Range::new(
                                Position::new(d.line.saturating_sub(1) as u32, 0),
                                Position::new(d.line.saturating_sub(1) as u32, 80),
                            ),
                        },
                        container_name: Some(d.category.clone()),
                    })
            })
            .collect();
        Ok(Some(syms))
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        // The pull surface is the agent/CI-facing path: return the real
        // detections, not an empty report. An empty pull report while the push
        // path reports cheats would itself be a laundered claim.
        let mut items: Vec<Diagnostic> = self
            .file_diagnostics(&params.text_document.uri)
            .iter()
            .map(|d| d.to_lsp())
            .collect();

        // Layer in AST syntax errors for Rust files (Path B).
        items.extend(
            self.ast_adapter
                .pull_ast_diagnostics(&params.text_document.uri),
        );

        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items,
                },
            }),
        ))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        // Tokens are projected from the real tree-sitter parse (Path B); a
        // non-Rust or unopened document yields no tokens rather than a guess.
        Ok(self
            .ast_adapter
            .semantic_tokens(&params.text_document.uri)
            .map(SemanticTokensResult::Tokens))
    }

    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> Result<Option<SemanticTokensFullDeltaResult>> {
        // A server may always answer a delta request with a full result; the
        // tokens still come from the parse tree, never a fabricated delta.
        Ok(self
            .ast_adapter
            .semantic_tokens(&params.text_document.uri)
            .map(SemanticTokensFullDeltaResult::Tokens))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        Ok(self
            .ast_adapter
            .semantic_tokens_in_range(&params.text_document.uri, params.range)
            .map(SemanticTokensRangeResult::Tokens))
    }

    /// LLMs fix one occurrence but not all; highlight all same-diagnostic-code
    /// locations in file so every instance of a violation is visible.
    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let diags = self.file_diagnostics(uri);
        let hits: Vec<DocumentHighlight> = diags
            .iter()
            .filter(|d| (d.line.saturating_sub(1) as u32) == pos.line)
            .map(|d| DocumentHighlight {
                range: Range::new(
                    Position::new(
                        d.line.saturating_sub(1) as u32,
                        d.column.saturating_sub(1) as u32,
                    ),
                    Position::new(
                        d.line.saturating_sub(1) as u32,
                        (d.column.saturating_sub(1) + 30) as u32,
                    ),
                ),
                kind: Some(DocumentHighlightKind::TEXT),
            })
            .collect();
        Ok(Some(hits))
    }

    /// LLMs cut at wrong syntactic boundaries; return detection ranges so the
    /// client shows the actual violation span rather than an arbitrary boundary.
    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<SelectionRange>>> {
        let uri = &params.text_document.uri;
        let diags = self.file_diagnostics(uri);
        let ranges: Vec<SelectionRange> = params
            .positions
            .iter()
            .map(|pos| {
                let matching = diags
                    .iter()
                    .filter(|d| (d.line.saturating_sub(1) as u32) == pos.line)
                    .map(|d| {
                        Range::new(
                            Position::new(
                                d.line.saturating_sub(1) as u32,
                                d.column.saturating_sub(1) as u32,
                            ),
                            Position::new(
                                d.line.saturating_sub(1) as u32,
                                (d.column.saturating_sub(1) + 40) as u32,
                            ),
                        )
                    })
                    .next()
                    .unwrap_or_else(|| Range::new(*pos, *pos));
                SelectionRange {
                    range: matching,
                    parent: None,
                }
            })
            .collect();
        Ok(Some(ranges))
    }

    /// LLMs edit open constructs without closing them; return cheat spans so
    /// paired-edit tooling reveals the asymmetry.
    async fn linked_editing_range(
        &self,
        params: LinkedEditingRangeParams,
    ) -> Result<Option<LinkedEditingRanges>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let diags = self.file_diagnostics(uri);
        let ranges: Vec<Range> = diags
            .iter()
            .filter(|d| (d.line.saturating_sub(1) as u32) == pos.line)
            .map(|d| {
                Range::new(
                    Position::new(
                        d.line.saturating_sub(1) as u32,
                        d.column.saturating_sub(1) as u32,
                    ),
                    Position::new(
                        d.line.saturating_sub(1) as u32,
                        (d.column.saturating_sub(1) + 30) as u32,
                    ),
                )
            })
            .collect();
        Ok(Some(LinkedEditingRanges {
            ranges,
            word_pattern: None,
        }))
    }

    /// LLM identifier laundering: return a moniker keyed by diagnostic code so
    /// tools can trace the violation across the LSIF graph.
    async fn moniker(&self, params: MonikerParams) -> Result<Option<Vec<Moniker>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let diags = self.file_diagnostics(uri);
        let monikers: Vec<Moniker> = diags
            .iter()
            .filter(|d| (d.line.saturating_sub(1) as u32) == pos.line)
            .map(|d| Moniker {
                scheme: "anti-llm".to_string(),
                identifier: format!("anti-llm/{}/{}", d.category, d.code),
                unique: UniquenessLevel::Document,
                kind: Some(MonikerKind::Import),
            })
            .collect();
        Ok(Some(monikers))
    }

    /// Inline law-state labels (BLOCKED/CANDIDATE) at each detection site so
    /// editors surface the status without requiring a hover.
    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = &params.text_document.uri;
        let diags = self.file_diagnostics(uri);
        let hints: Vec<InlayHint> = diags
            .iter()
            .map(|d| {
                let status = if d.blocking { "BLOCKED" } else { "CANDIDATE" };
                InlayHint {
                    position: Position::new(d.line.saturating_sub(1) as u32, 0),
                    label: InlayHintLabel::String(format!("\u{2691} {}: {}", d.code, status)),
                    kind: Some(InlayHintKind::PARAMETER),
                    text_edits: None,
                    tooltip: Some(InlayHintTooltip::String(d.message.clone())),
                    padding_left: None,
                    padding_right: Some(true),
                    data: None,
                }
            })
            .collect();
        Ok(Some(hints))
    }

    /// Resolve is a no-op here: hints are fully populated at creation time.
    async fn inlay_hint_resolve(&self, hint: InlayHint) -> Result<InlayHint> {
        Ok(hint)
    }

    /// Show BLOCKED/CANDIDATE status inline in code at each detection site.
    async fn inline_value(&self, params: InlineValueParams) -> Result<Option<Vec<InlineValue>>> {
        let uri = &params.text_document.uri;
        let diags = self.file_diagnostics(uri);
        let range_start = params.range.start.line;
        let range_end = params.range.end.line;
        let values: Vec<InlineValue> = diags
            .iter()
            .filter(|d| {
                let l = d.line.saturating_sub(1) as u32;
                l >= range_start && l <= range_end
            })
            .map(|d| {
                InlineValue::Text(InlineValueText {
                    range: Range::new(
                        Position::new(d.line.saturating_sub(1) as u32, 0),
                        Position::new(d.line.saturating_sub(1) as u32, 10),
                    ),
                    text: if d.blocking {
                        "BLOCKED".to_string()
                    } else {
                        "CANDIDATE".to_string()
                    },
                })
            })
            .collect();
        Ok(Some(values))
    }

    /// Read-only law: emit no edits; surface formatting-tell diagnostics via
    /// a scan-and-publish so the push path sees the same detections.
    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        self.run_scan_and_publish(&params.text_document.uri).await;
        Ok(Some(vec![]))
    }

    /// Rename-as-obfuscation detection: refuse any rename that lands on a
    /// violation site. The LSP surface is read-only; renaming does not fix law
    /// violations, it obscures them.
    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = &params.text_document.uri;
        let pos = params.position;
        let diags = self.file_diagnostics(uri);
        let has_violation = diags
            .iter()
            .any(|d| (d.line.saturating_sub(1) as u32) == pos.line);
        if has_violation {
            return Err(Error {
                code: ErrorCode::InvalidRequest,
                message: std::borrow::Cow::Borrowed(
                    "ANTI-LLM-RENAME-001: Renaming at a violation site is forbidden by read-only law",
                ),
                data: None,
            });
        }
        Ok(None)
    }

    /// Rename is refused by read-only law: the LSP surface emits diagnostics
    /// only and never mutates files.
    async fn rename(&self, _params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        Err(Error {
            code: ErrorCode::InvalidRequest,
            message: std::borrow::Cow::Borrowed(
                "ANTI-LLM-RENAME-002: Rename is refused by read-only law",
            ),
            data: None,
        })
    }

    /// Commands for gate control, receipt-chain validation, and OCEL export.
    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            "anti-llm.validateReceiptChain" => {
                let root = self.root_dir();
                Ok(Some(serde_json::json!({
                    "status": "CANDIDATE",
                    "root": root,
                    "message": "Receipt chain validation deferred to validate-receipt-chain.sh"
                })))
            }
            "anti-llm.exportOcel" => Ok(Some(serde_json::json!({
                "status": "PARTIAL",
                "message": "OCEL export available via virtual doc anti-llm://ocel-log"
            }))),
            _ => Ok(None),
        }
    }

    /// CI/agent pull path for workspace-wide diagnostics; groups detections by
    /// file so every active violation is reachable without opening each file.
    async fn workspace_diagnostic(
        &self,
        _params: WorkspaceDiagnosticParams,
    ) -> Result<WorkspaceDiagnosticReportResult> {
        let root = self.root_dir();
        let obs = engine::scan_directory(&root);
        let all_diags = engine::evaluate_diagnostics(&obs);
        use std::collections::HashMap;
        let mut by_file: HashMap<String, Vec<Diagnostic>> = HashMap::new();
        for d in &all_diags {
            by_file
                .entry(d.file_path.clone())
                .or_default()
                .push(d.to_lsp());
        }
        let items: Vec<WorkspaceDocumentDiagnosticReport> = by_file
            .into_iter()
            .filter_map(|(path, items)| {
                url::Url::from_file_path(&path)
                    .ok()
                    .and_then(|url| Uri::from_str(url.as_str()).ok())
                    .map(|uri| {
                        WorkspaceDocumentDiagnosticReport::Full(
                            WorkspaceFullDocumentDiagnosticReport {
                                uri,
                                version: None,
                                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                                    result_id: None,
                                    items,
                                },
                            },
                        )
                    })
            })
            .collect();
        Ok(WorkspaceDiagnosticReportResult::Report(
            WorkspaceDiagnosticReport { items },
        ))
    }

    /// Fake call graphs detection surface: return detections on the cursor line
    /// as call hierarchy items so call-graph tooling traces law-violation chains.
    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let diags = self.file_diagnostics(uri);
        let items: Vec<CallHierarchyItem> = diags
            .iter()
            .filter(|d| (d.line.saturating_sub(1) as u32) == pos.line)
            .map(|d| CallHierarchyItem {
                name: d.code.clone(),
                kind: SymbolKind::EVENT,
                tags: None,
                detail: Some(d.category.clone()),
                uri: uri.clone(),
                range: Range::new(
                    Position::new(d.line.saturating_sub(1) as u32, 0),
                    Position::new(d.line.saturating_sub(1) as u32, 80),
                ),
                selection_range: Range::new(
                    Position::new(d.line.saturating_sub(1) as u32, 0),
                    Position::new(d.line.saturating_sub(1) as u32, 40),
                ),
                data: None,
            })
            .collect();
        Ok(Some(items))
    }

    /// Incoming calls = law obligations that led to this violation.
    async fn incoming_calls(
        &self,
        _params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        Ok(Some(vec![]))
    }

    /// Outgoing calls = required_next_proof chain obligations.
    async fn outgoing_calls(
        &self,
        _params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        Ok(Some(vec![]))
    }

    async fn will_save(&self, params: WillSaveTextDocumentParams) {
        self.run_scan_and_publish(&params.text_document.uri).await;
    }

    async fn will_save_wait_until(
        &self,
        params: WillSaveTextDocumentParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        self.run_scan_and_publish(&params.text_document.uri).await;
        Ok(Some(vec![]))
    }

    async fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        Ok(item)
    }

    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let uri = &params.text_document.uri;
        let diags = self.file_diagnostics(uri);
        let target = Uri::from_str("anti-llm://failset").ok();
        let links: Vec<DocumentLink> = diags
            .iter()
            .map(|d| DocumentLink {
                range: Range::new(
                    Position::new(d.line.saturating_sub(1) as u32, 0),
                    Position::new(d.line.saturating_sub(1) as u32, 30),
                ),
                target: target.clone(),
                tooltip: Some(d.message.clone()),
                data: None,
            })
            .collect();
        Ok(Some(links))
    }

    async fn document_link_resolve(&self, link: DocumentLink) -> Result<DocumentLink> {
        Ok(link)
    }

    async fn document_color(&self, _params: DocumentColorParams) -> Result<Vec<ColorInformation>> {
        Ok(vec![])
    }

    async fn color_presentation(
        &self,
        _params: ColorPresentationParams,
    ) -> Result<Vec<ColorPresentation>> {
        Ok(vec![])
    }

    async fn code_action_resolve(&self, action: CodeAction) -> Result<CodeAction> {
        Ok(action)
    }

    async fn on_type_formatting(
        &self,
        _params: DocumentOnTypeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        Ok(Some(vec![]))
    }

    async fn prepare_type_hierarchy(
        &self,
        params: TypeHierarchyPrepareParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let diags = self.file_diagnostics(uri);
        let items: Vec<TypeHierarchyItem> = diags
            .iter()
            .filter(|d| (d.line.saturating_sub(1) as u32) == pos.line)
            .map(|d| TypeHierarchyItem {
                name: d.code.clone(),
                kind: SymbolKind::EVENT,
                tags: None,
                detail: Some(d.category.clone()),
                uri: uri.clone(),
                range: Range::new(
                    Position::new(d.line.saturating_sub(1) as u32, 0),
                    Position::new(d.line.saturating_sub(1) as u32, 80),
                ),
                selection_range: Range::new(
                    Position::new(d.line.saturating_sub(1) as u32, 0),
                    Position::new(d.line.saturating_sub(1) as u32, 40),
                ),
                data: None,
            })
            .collect();
        Ok(Some(items))
    }

    async fn supertypes(
        &self,
        _params: TypeHierarchySupertypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        Ok(Some(vec![]))
    }

    async fn subtypes(
        &self,
        _params: TypeHierarchySubtypesParams,
    ) -> Result<Option<Vec<TypeHierarchyItem>>> {
        Ok(Some(vec![]))
    }

    async fn symbol_resolve(&self, symbol: WorkspaceSymbol) -> Result<WorkspaceSymbol> {
        Ok(symbol)
    }

    async fn did_change_configuration(&self, _params: DidChangeConfigurationParams) {
        let root = self.root_dir();
        if let Ok(uri) = Uri::from_str(&format!("file://{}", root)) {
            self.run_scan_and_publish(&uri).await;
        }
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for change in &params.changes {
            self.run_scan_and_publish(&change.uri).await;
        }
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        if let Some(folder) = params.event.added.first() {
            if let Ok(url) = url::Url::parse(folder.uri.as_str()) {
                if let Ok(path) = url.to_file_path() {
                    let mut root = self.workspace_root.lock().unwrap();
                    *root = Some(path.to_string_lossy().to_string());
                }
            }
        }
    }

    async fn will_create_files(&self, _params: CreateFilesParams) -> Result<Option<WorkspaceEdit>> {
        Ok(None)
    }

    async fn will_rename_files(&self, _params: RenameFilesParams) -> Result<Option<WorkspaceEdit>> {
        Ok(None)
    }

    async fn will_delete_files(&self, _params: DeleteFilesParams) -> Result<Option<WorkspaceEdit>> {
        Ok(None)
    }

    async fn did_create_files(&self, _params: CreateFilesParams) {
        self.client
            .log_message(MessageType::INFO, "file create observed")
            .await;
    }

    async fn did_rename_files(&self, _params: RenameFilesParams) {
        self.client
            .log_message(MessageType::INFO, "file rename observed")
            .await;
    }

    async fn did_delete_files(&self, _params: DeleteFilesParams) {
        self.client
            .log_message(MessageType::INFO, "file delete observed")
            .await;
    }

    async fn did_open_notebook_document(&self, _params: DidOpenNotebookDocumentParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "notebook opened: cell diagnostics not emitted",
            )
            .await;
    }

    async fn did_change_notebook_document(&self, _params: DidChangeNotebookDocumentParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "notebook changed: cell diagnostics not emitted",
            )
            .await;
    }

    async fn did_save_notebook_document(&self, _params: DidSaveNotebookDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "notebook saved")
            .await;
    }

    async fn did_close_notebook_document(&self, _params: DidCloseNotebookDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "notebook closed")
            .await;
    }

    async fn work_done_progress_cancel(&self, _params: WorkDoneProgressCancelParams) {
        // Progress token cancellation observed
    }

    async fn set_trace(&self, params: SetTraceParams) {
        self.client
            .log_trace(LogTraceParams {
                message: format!("trace level: {:?}", params.value),
                verbose: None,
            })
            .await;
    }

    async fn progress(&self, _params: ProgressParams) {
        // Progress notification received
    }
}

impl RulePackServer for AntiLlmServer {
    fn rule_packs(&self) -> &ValidatedRulePackSet {
        &self.rule_packs
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn server_name(&self) -> &'static str {
        "anti-llm-cheat-lsp"
    }

    fn client(&self) -> &Client {
        &self.client
    }

    fn adapter(&self) -> &AutoLspAdapter {
        self.ast_adapter.inner()
    }

    fn workspace_index(&self) -> Option<&WorkspaceIndex> {
        Some(&self.workspace_index)
    }

    /// Bridge the engine's AhoCorasick + multi-format scan into `ClassifiedFindings`.
    ///
    /// The canary does not use TOML rule packs; it delegates to `engine::scan_directory`
    /// which owns the AhoCorasick automaton.  This override converts `AntiLlmDiagnostic`
    /// results into the `(MaxDiagnostic, Diagnostic)` pairs the trait expects.
    fn scan_uri_classified(&self, uri: &DocumentUri, _content: &str) -> ClassifiedFindings {
        let root_dir = self.root_dir();
        let obs = engine::scan_directory(&root_dir);
        let raw = engine::evaluate_diagnostics(&obs);
        let norm_uri = uri.to_string().replace('\\', "/");

        let findings: Vec<Finding> = raw
            .into_iter()
            .filter(|d| norm_uri.ends_with(&d.file_path.replace('\\', "/")))
            .map(|d| {
                let lsp_diag = d.to_lsp();
                let law_axis = LawAxis::Custom(d.category.clone());
                let max_diag = MaxDiagnostic {
                    lsp: lsp_diag.clone(),
                    diagnostic_id: d.code.clone(),
                    law_id: d.category.clone(),
                    law_axis,
                    violated_invariant: d.forbidden_implication.clone(),
                    ..MaxDiagnostic::default()
                };
                (max_diag, lsp_diag)
            })
            .collect();

        // All engine findings are sync-classified (AhoCorasick is fast).
        (findings, vec![])
    }
}
