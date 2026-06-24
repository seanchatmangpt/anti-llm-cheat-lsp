use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub file_path: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub column: usize,
    pub kind: String,      // e.g., "raw_text", "ast_node", "manifest_dep"
    pub construct: String, // e.g., "unwrap()", "tower-lsp", "CLAP"
    pub context: String,   // surrounding context or function name
    pub message: String,
}
