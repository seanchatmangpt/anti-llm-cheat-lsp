//! AST-driven incremental features via AutoLspAdapter.
//!
//! This module integrates lsp-max-ast's incremental tree-sitter document store
//! for Rust-specific AST features (syntax error diagnostics, document symbols).
//! The adapter layers on top of the existing multi-format engine without
//! displacing cross-file law detection.

use lsp_max::lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentUri,
};
use lsp_max_ast::AutoLspAdapter;

/// Wraps AutoLspAdapter with Rust language binding.
pub struct RustAstAdapter {
    adapter: AutoLspAdapter,
}

impl RustAstAdapter {
    /// Create a new Rust AST adapter.
    pub fn new() -> Self {
        Self {
            adapter: AutoLspAdapter::new_default(),
        }
    }

    /// Return true if the URI refers to a Rust file.
    fn is_rust_file(uri: &DocumentUri) -> bool {
        uri.as_str().ends_with(".rs")
    }

    /// Forward did_open to adapter if it's a Rust file.
    pub fn handle_did_open(&self, params: DidOpenTextDocumentParams) {
        if Self::is_rust_file(&params.text_document.uri) {
            self.adapter
                .handle_did_open(params, tree_sitter_rust::LANGUAGE.into());
        }
    }

    /// Forward did_change to adapter if it's a Rust file.
    pub fn handle_did_change(&self, params: DidChangeTextDocumentParams) {
        if Self::is_rust_file(&params.text_document.uri) {
            self.adapter
                .handle_did_change(params, tree_sitter_rust::LANGUAGE.into());
        }
    }

    /// Forward did_close to adapter.
    pub fn handle_did_close(&self, params: DidCloseTextDocumentParams) {
        self.adapter.handle_did_close(params);
    }

    /// Get AST syntax error diagnostics for a Rust file.
    pub fn pull_ast_diagnostics(&self, uri: &DocumentUri) -> Vec<lsp_types_max::Diagnostic> {
        if Self::is_rust_file(uri) {
            self.adapter.pull_diagnostics(uri)
        } else {
            Vec::new()
        }
    }

    /// Return a reference to the underlying `AutoLspAdapter` for use in
    /// `RulePackServer::adapter()` implementations that require the raw adapter.
    pub fn inner(&self) -> &AutoLspAdapter {
        &self.adapter
    }

    pub fn get_document<F, R>(&self, uri: &DocumentUri, f: F) -> Option<R>
    where
        F: FnOnce(&lsp_max_ast_core::document::Document) -> R,
    {
        if Self::is_rust_file(uri) {
            self.adapter.get_document(uri, f)
        } else {
            None
        }
    }

    /// AST-derived semantic tokens for a Rust file, or `None` if the document is
    /// not a tracked Rust file.
    pub fn semantic_tokens(&self, uri: &DocumentUri) -> Option<lsp_max::lsp_types::SemanticTokens> {
        self.get_document(uri, crate::semantic::build_tokens)
    }

    /// AST-derived semantic tokens restricted to `range`.
    pub fn semantic_tokens_in_range(
        &self,
        uri: &DocumentUri,
        range: lsp_max::lsp_types::Range,
    ) -> Option<lsp_max::lsp_types::SemanticTokens> {
        self.get_document(uri, |doc| {
            crate::semantic::build_tokens_in_range(doc, range)
        })
    }
}

impl Default for RustAstAdapter {
    fn default() -> Self {
        Self::new()
    }
}
