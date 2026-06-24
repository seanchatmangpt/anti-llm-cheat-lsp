//! AST-derived semantic tokens for Rust files.
//!
//! Path B gave the server an incremental tree-sitter document store; this module
//! turns that parse tree into LSP semantic tokens. The legend is declared once
//! here and consumed by both `capabilities` (advertisement) and the handler
//! (emission) so the two can never disagree.
//!
//! Tokens are derived from real syntax-tree leaves — never invented — so a
//! `textDocument/semanticTokens/*` response reports what the formal parser
//! actually observed. Multiline leaves (e.g. block comments spanning lines) are
//! skipped because a single semantic token cannot cross a line boundary.

use lsp_max::lsp_types::{
    Range, SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};
use lsp_max_ast_core::document::Document;

/// The token-type legend, indexed by position. The handler emits indices into
/// this array; `capabilities` advertises the identical legend.
pub const TOKEN_TYPES: [SemanticTokenType; 8] = [
    SemanticTokenType::KEYWORD,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::TYPE,
    SemanticTokenType::STRING,
    SemanticTokenType::COMMENT,
    SemanticTokenType::NUMBER,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::PROPERTY,
];

const T_KEYWORD: u32 = 0;
const T_FUNCTION: u32 = 1;
const T_TYPE: u32 = 2;
const T_STRING: u32 = 3;
const T_COMMENT: u32 = 4;
const T_NUMBER: u32 = 5;
const T_VARIABLE: u32 = 6;
const T_PROPERTY: u32 = 7;

/// The legend advertised in `SemanticTokensOptions`.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: Vec::new(),
    }
}

/// Classify a syntax-tree leaf into a legend index, or `None` if it carries no
/// highlight. Plain `identifier` leaves are disambiguated by parent context so a
/// function name/call reads as `function`, everything else as `variable`.
fn classify(node: &tree_sitter::Node) -> Option<u32> {
    if node.kind() == "identifier" {
        let parent_kind = node.parent().map(|p| p.kind());
        return Some(match parent_kind {
            Some("function_item") | Some("function_signature_item") | Some("call_expression") => {
                T_FUNCTION
            }
            _ => T_VARIABLE,
        });
    }
    token_type_for(node.kind())
}

/// Map a tree-sitter Rust node kind to a legend index, or `None` if the node
/// carries no highlight. Keyword literals surface as anonymous tokens whose
/// `kind` is the literal itself.
fn token_type_for(kind: &str) -> Option<u32> {
    match kind {
        "type_identifier" | "primitive_type" => Some(T_TYPE),
        "field_identifier" | "shorthand_field_identifier" => Some(T_PROPERTY),
        "string_literal" | "raw_string_literal" | "char_literal" => Some(T_STRING),
        "line_comment" | "block_comment" => Some(T_COMMENT),
        "integer_literal" | "float_literal" => Some(T_NUMBER),
        // Anonymous keyword tokens in tree-sitter-rust carry the literal as kind.
        "fn" | "let" | "mut" | "pub" | "struct" | "enum" | "impl" | "trait" | "use" | "mod"
        | "const" | "static" | "if" | "else" | "match" | "for" | "while" | "loop" | "return"
        | "self" | "crate" | "super" | "as" | "where" | "async" | "await" | "move" | "ref"
        | "dyn" | "unsafe" | "extern" | "type" | "in" | "break" | "continue" => Some(T_KEYWORD),
        _ => None,
    }
}

/// Walk the parse tree and produce semantic tokens for the whole document.
pub fn build_tokens(doc: &Document) -> SemanticTokens {
    let collected = collect(doc, None);
    encode(collected)
}

/// Produce semantic tokens restricted to `range` (used by the range request).
pub fn build_tokens_in_range(doc: &Document, range: Range) -> SemanticTokens {
    let collected = collect(doc, Some(range));
    encode(collected)
}

/// Collect `(lsp_range, token_type)` pairs for every highlightable leaf, in
/// source order. `clip` optionally restricts to leaves starting within a range.
fn collect(doc: &Document, clip: Option<Range>) -> Vec<(Range, u32)> {
    let mut cursor = doc.tree.walk();
    let mut out: Vec<(Range, u32)> = Vec::new();

    // Iterative pre-order traversal over the cursor.
    loop {
        let node = cursor.node();
        if node.child_count() == 0 {
            if let Some(tt) = classify(&node) {
                if let Ok(r) = doc.denormalize_range(&node.range()) {
                    // A semantic token may not span lines.
                    if r.start.line == r.end.line {
                        let keep = clip
                            .as_ref()
                            .map(|c| r.start.line >= c.start.line && r.end.line <= c.end.line)
                            .unwrap_or(true);
                        if keep {
                            out.push((r, tt));
                        }
                    }
                }
            }
        }

        // Descend, else move to next sibling, else climb until a sibling exists.
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                out.sort_by_key(|(r, _)| (r.start.line, r.start.character));
                return out;
            }
        }
    }
}

/// Delta-encode ordered tokens per the LSP semantic-tokens wire format.
fn encode(tokens: Vec<(Range, u32)>) -> SemanticTokens {
    let mut data = Vec::with_capacity(tokens.len());
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;
    for (r, tt) in tokens {
        let line = r.start.line;
        let ch = r.start.character;
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 { ch - prev_char } else { ch };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: r.end.character.saturating_sub(r.start.character),
            token_type: tt,
            token_modifiers_bitset: 0,
        });
        prev_line = line;
        prev_char = ch;
    }
    SemanticTokens {
        result_id: None,
        data,
    }
}

#[cfg(test)]
mod witness {
    use super::*;
    use tree_sitter::Parser;

    fn doc(src: &str) -> Document {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("rust grammar");
        let tree = parser.parse(src, None).expect("parse");
        Document::new(src.to_string(), tree, None)
    }

    #[test]
    fn tokens_are_parse_derived_not_fabricated() {
        // A `Wired` semantic-tokens row must emit tokens that correspond to real
        // syntax-tree leaves; an empty or invented response would be the hollow
        // handler the canary refuses.
        let d = doc("fn main() {\n    let x = 42;\n}\n");
        let toks = build_tokens(&d);
        assert!(
            !toks.data.is_empty(),
            "parse tree has keywords/identifiers/numbers; tokens must be non-empty"
        );
        // The legend index emitted must be in range of the advertised legend.
        for t in &toks.data {
            assert!(
                (t.token_type as usize) < TOKEN_TYPES.len(),
                "token_type index escapes the advertised legend"
            );
        }
    }

    #[test]
    fn keyword_and_number_classified() {
        let d = doc("const N: u32 = 7;\n");
        let toks = build_tokens(&d);
        let types: Vec<u32> = toks.data.iter().map(|t| t.token_type).collect();
        assert!(
            types.contains(&T_KEYWORD),
            "`const` must classify as keyword"
        );
        assert!(types.contains(&T_NUMBER), "`7` must classify as number");
        assert!(types.contains(&T_TYPE), "`u32` must classify as type");
    }

    #[test]
    fn function_name_classified_via_parent_context() {
        let d = doc("fn compute() {}\n");
        let toks = build_tokens(&d);
        let types: Vec<u32> = toks.data.iter().map(|t| t.token_type).collect();
        assert!(
            types.contains(&T_FUNCTION),
            "`compute` (function_item name) must classify as function, not variable"
        );
    }

    #[test]
    fn range_restriction_is_a_subset() {
        let src = "fn a() {}\nfn b() {}\nfn c() {}\n";
        let d = doc(src);
        let full = build_tokens(&d).data.len();
        let first_line = build_tokens_in_range(
            &d,
            Range::new(
                lsp_max::lsp_types::Position::new(0, 0),
                lsp_max::lsp_types::Position::new(0, 9),
            ),
        )
        .data
        .len();
        assert!(
            first_line < full && first_line > 0,
            "range tokens ({first_line}) must be a non-empty subset of full ({full})"
        );
    }
}
