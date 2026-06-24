use crate::diagnostics::AntiLlmDiagnostic;
use crate::observations::Observation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lsp318Feature {
    pub feature_id: String,
    pub feature: String,
    pub status: String,
    pub client_capability_path: String,
    pub server_capability_path: String,
    pub request_method: String,
    pub response_or_notification_method: String,
    pub positive_transcript_path: String,
    pub negative_control_path: String,
    pub receipt_path: String,
    pub digest: String,
    pub forbidden_substitution_prevented: String,
}

pub fn get_feature_matrix() -> Vec<Lsp318Feature> {
    vec![
        Lsp318Feature {
            feature_id: "LSP318-001".to_string(),
            feature: "Inline completions".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.textDocument.inlineCompletion".to_string(),
            server_capability_path: "capabilities.inlineCompletionProvider".to_string(),
            request_method: "textDocument/inlineCompletion".to_string(),
            response_or_notification_method: "textDocument/inlineCompletion".to_string(),
            positive_transcript_path: "transcripts/inline_completion_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/no_victory_language.rs".to_string(),
            receipt_path: "receipts/inline_completion_receipt.json".to_string(),
            digest: "5a17d2a04d2eafa43a3eada0d0afb1a4830b6620d8a18182294ec55567d83c67".to_string(),
            forbidden_substitution_prevented: "Victory language overclaim bypass".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-002".to_string(),
            feature: "Dynamic text document content".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.workspace.textDocumentContent".to_string(),
            server_capability_path: "capabilities.textDocumentContentProvider".to_string(),
            request_method: "workspace/textDocumentContent".to_string(),
            response_or_notification_method: "workspace/textDocumentContent".to_string(),
            positive_transcript_path: "transcripts/text_document_content_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/invalid_virtual_uri.txt".to_string(),
            receipt_path: "receipts/text_document_content_receipt.json".to_string(),
            digest: "a0d22554ce386f6202abe085091d1e19799f14684992e9c4322682d3be1e66ac".to_string(),
            forbidden_substitution_prevented: "Static mock file substitution".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-003".to_string(),
            feature: "Folding range refresh".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.workspace.foldingRange.refresh".to_string(),
            server_capability_path: "capabilities.foldingRangeProvider".to_string(),
            request_method: "workspace/foldingRange/refresh".to_string(),
            response_or_notification_method: "workspace/foldingRange/refresh".to_string(),
            positive_transcript_path: "transcripts/folding_range_refresh_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/no_refresh_trigger.txt".to_string(),
            receipt_path: "receipts/folding_range_refresh_receipt.json".to_string(),
            digest: "85705ceef65aeb119bd05a4ffd5464560c681cde6401458f43872d697df501ec".to_string(),
            forbidden_substitution_prevented: "Stale folding structure".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-004".to_string(),
            feature: "Multi-range formatting".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.textDocument.formatting.rangesSupport".to_string(),
            server_capability_path: "capabilities.documentRangeFormattingProvider".to_string(),
            request_method: "textDocument/rangesFormatting".to_string(),
            response_or_notification_method: "textDocument/rangesFormatting".to_string(),
            positive_transcript_path: "transcripts/ranges_formatting_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/single_range_formatting.rs".to_string(),
            receipt_path: "receipts/ranges_formatting_receipt.json".to_string(),
            digest: "7b29e0860f3c27c7713084be3e07d048a467186c96f0b864b0e737f11017f32e".to_string(),
            forbidden_substitution_prevented: "Single-range formatting substitution".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-005".to_string(),
            feature: "Snippets in workspace edits".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.workspace.workspaceEdit.changeAnnotationSupport".to_string(),
            server_capability_path: "capabilities.workspace.workspaceEdit".to_string(),
            request_method: "workspace/applyEdit".to_string(),
            response_or_notification_method: "workspace/applyEdit".to_string(),
            positive_transcript_path: "transcripts/workspace_edit_snippets_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/authority_mutation_snippets.rs".to_string(),
            receipt_path: "receipts/workspace_edit_snippets_receipt.json".to_string(),
            digest: "2ba74abb85a1b1be5012ef63ac3599e19cbaa4dfc9950f85f14ff73b24eec9cf".to_string(),
            forbidden_substitution_prevented: "Direct authority file mutation".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-006".to_string(),
            feature: "Relative patterns in document filters".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.textDocument.synchronization.dynamicRegistration".to_string(),
            server_capability_path: "capabilities.textDocumentSync".to_string(),
            request_method: "client/registerCapability".to_string(),
            response_or_notification_method: "client/registerCapability".to_string(),
            positive_transcript_path: "transcripts/relative_patterns_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/global_pattern_scope.rs".to_string(),
            receipt_path: "receipts/relative_patterns_receipt.json".to_string(),
            digest: "d64a4d10d530fbdf92e9301aa5ee3179b539df7d6b205ba8b75d409553b64175".to_string(),
            forbidden_substitution_prevented: "Global sloppy file scanning".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-007".to_string(),
            feature: "Relative patterns in notebook document filters".to_string(),
            status: "REFUSED_BY_LAW_WITH_RECEIPT".to_string(),
            client_capability_path: "capabilities.notebookDocument.synchronization".to_string(),
            server_capability_path: "capabilities.notebookDocumentSync".to_string(),
            request_method: "none".to_string(),
            response_or_notification_method: "none".to_string(),
            positive_transcript_path: "none".to_string(),
            negative_control_path: "none".to_string(),
            receipt_path: "receipts/notebook_refusal_receipt.json".to_string(),
            digest: "82e7b0f46108a93673fbd39c391970325529ab87dc2892db287001257a30255f".to_string(),
            forbidden_substitution_prevented: "Notebook document out-of-scope bypass".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-008".to_string(),
            feature: "Code action kind documentation".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.textDocument.codeAction.codeActionLiteralSupport.codeActionKind.valueSet".to_string(),
            server_capability_path: "capabilities.codeActionProvider.codeActionKinds".to_string(),
            request_method: "textDocument/codeAction".to_string(),
            response_or_notification_method: "textDocument/codeAction".to_string(),
            positive_transcript_path: "transcripts/code_action_kind_doc_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/undocumented_code_action.rs".to_string(),
            receipt_path: "receipts/code_action_kind_doc_receipt.json".to_string(),
            digest: "94355bedf5c10c80445a54f0f839bd84487f99eae3af034d0fa65f6144602cee".to_string(),
            forbidden_substitution_prevented: "Undocumented code actions".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-009".to_string(),
            feature: "Nullable activeParameter on SignatureHelp".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.textDocument.signatureHelp.signatureInformation.activeParameterSupport".to_string(),
            server_capability_path: "capabilities.signatureHelpProvider".to_string(),
            request_method: "textDocument/signatureHelp".to_string(),
            response_or_notification_method: "textDocument/signatureHelp".to_string(),
            positive_transcript_path: "transcripts/nullable_active_parameter_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/invalid_non_null_parameter.rs".to_string(),
            receipt_path: "receipts/nullable_active_parameter_receipt.json".to_string(),
            digest: "87ec9c979c6edb1df600577aee9326caa66402e9620908a638ce7c82c8bd0c6f".to_string(),
            forbidden_substitution_prevented: "Invalid parameter type parser panic".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-010".to_string(),
            feature: "Command tooltips".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.window.showDocument".to_string(),
            server_capability_path: "capabilities.executeCommandProvider.commands".to_string(),
            request_method: "workspace/executeCommand".to_string(),
            response_or_notification_method: "workspace/executeCommand".to_string(),
            positive_transcript_path: "transcripts/command_tooltips_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/no_tooltip_command.rs".to_string(),
            receipt_path: "receipts/command_tooltips_receipt.json".to_string(),
            digest: "d46cbb4aeb7a67ac8a12b6bad12d37343bf257f0fdc2cdf165b59813cf80f79a".to_string(),
            forbidden_substitution_prevented: "Undocumented command execution".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-011".to_string(),
            feature: "Workspace edit metadata".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.workspace.workspaceEdit.metadataSupport".to_string(),
            server_capability_path: "capabilities.workspace.workspaceEdit".to_string(),
            request_method: "workspace/applyEdit".to_string(),
            response_or_notification_method: "workspace/applyEdit".to_string(),
            positive_transcript_path: "transcripts/workspace_edit_metadata_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/workspace_edit_no_metadata.rs".to_string(),
            receipt_path: "receipts/workspace_edit_metadata_receipt.json".to_string(),
            digest: "52811befc9d6dd6b306085912acdc2edfe8002739854abe5c4c391dc06438723".to_string(),
            forbidden_substitution_prevented: "Unlabeled workspace mutation".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-012".to_string(),
            feature: "Snippets in text document edits".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.textDocument.completion.completionItem.snippetSupport".to_string(),
            server_capability_path: "capabilities.completionProvider".to_string(),
            request_method: "textDocument/completion".to_string(),
            response_or_notification_method: "textDocument/completion".to_string(),
            positive_transcript_path: "transcripts/text_document_edit_snippets_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/production_snippet_mutation.rs".to_string(),
            receipt_path: "receipts/text_document_edit_snippets_receipt.json".to_string(),
            digest: "8f6da116660d478e312bb12189cf19c9f5e8018b59eba8ea612964048f2e006d".to_string(),
            forbidden_substitution_prevented: "Production path mutation snippet".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-013".to_string(),
            feature: "Debug message kind".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.window.showMessage.messageActionItem".to_string(),
            server_capability_path: "none".to_string(),
            request_method: "window/logMessage".to_string(),
            response_or_notification_method: "window/logMessage".to_string(),
            positive_transcript_path: "transcripts/debug_message_kind_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/debug_log_as_diagnostic.rs".to_string(),
            receipt_path: "receipts/debug_message_kind_receipt.json".to_string(),
            digest: "fbae4eff2fe6b2435088ac4dd986bd4082125d6126aa78aa6f99cbfe135def00".to_string(),
            forbidden_substitution_prevented: "Diagnostic leak in debug logs".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-014".to_string(),
            feature: "Code lens resolvable properties".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.textDocument.codeLens.dynamicRegistration".to_string(),
            server_capability_path: "capabilities.codeLensProvider.resolveProvider".to_string(),
            request_method: "codeLens/resolve".to_string(),
            response_or_notification_method: "codeLens/resolve".to_string(),
            positive_transcript_path: "transcripts/code_lens_resolve_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/unresolvable_properties.rs".to_string(),
            receipt_path: "receipts/code_lens_resolve_receipt.json".to_string(),
            digest: "d834ee12588c4156c5723f60b8654e23fda3c3a65e7c0d831b7d8d5e26fb06bb".to_string(),
            forbidden_substitution_prevented: "Undeclared code lens properties resolve".to_string(),
        },
        Lsp318Feature {
            feature_id: "LSP318-015".to_string(),
            feature: "completionList.applyKind".to_string(),
            status: "SUPPORTED_WITH_TRANSCRIPT".to_string(),
            client_capability_path: "capabilities.textDocument.completion.completionList.itemDefaults".to_string(),
            server_capability_path: "capabilities.completionProvider.completionItem".to_string(),
            request_method: "textDocument/completion".to_string(),
            response_or_notification_method: "textDocument/completion".to_string(),
            positive_transcript_path: "transcripts/completion_list_apply_kind_positive.jsonl".to_string(),
            negative_control_path: "fixtures/negative_controls/no_apply_kind_completion.rs".to_string(),
            receipt_path: "receipts/completion_list_apply_kind_receipt.json".to_string(),
            digest: "a9c7d9c4ea02b49dc60bc3a9c43c96e7025cc423000d7a1bdc2597b136b23f33".to_string(),
            forbidden_substitution_prevented: "Sloppy completion item defaults laundering".to_string(),
        },
    ]
}

pub fn evaluate(obs: &[Observation]) -> Vec<AntiLlmDiagnostic> {
    let mut diags = Vec::new();

    for o in obs {
        // Skip rules and engine definitions to avoid self-triggering
        if o.file_path.ends_with("lsp318.rs") || o.file_path.ends_with("engine.rs") {
            continue;
        }

        let is_laundering = o
            .construct
            .contains("ChangelogCoverage(15 rows) => SpecCoverage(LSP 3.18)")
            || o.construct
                .contains("ChangelogCoverage(15 rows) \u{21d2} SpecCoverage(LSP 3.18)")
            || o.construct.contains(
                "15-row changelog matrix is being treated as full LSP 3.18 combinatorial coverage",
            )
            || o.context
                .contains("ChangelogCoverage(15 rows) => SpecCoverage(LSP 3.18)")
            || o.context
                .contains("ChangelogCoverage(15 rows) \u{21d2} SpecCoverage(LSP 3.18)")
            || o.context.contains(
                "15-row changelog matrix is being treated as full LSP 3.18 combinatorial coverage",
            );

        if is_laundering {
            diags.push(AntiLlmDiagnostic {
                code: "ANTI-LLM-LSP318-COMB-001".to_string(),
                category: "protocol_surface".to_string(),
                file_path: o.file_path.clone(),
                line: o.line,
                column: o.column,
                message: "15-row changelog matrix is being treated as full LSP 3.18 combinatorial coverage.".to_string(),
                forbidden_implication: "ChangelogCoverage(15 rows) \u{21d2} SpecCoverage(LSP 3.18)".to_string(),
                blocking: true,
                required_correction: "Acknowledge that 15-row matrix is delta changelog coverage only, and implement spec extractor to generate complete combinatorial coverage.".to_string(),
                required_next_proof: "Run the spec extractor script to generate the matrix and gap report, proving real protocol graph mapping.".to_string(),
            });
        }
    }

    diags
}
