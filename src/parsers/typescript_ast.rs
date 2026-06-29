// typescript_ast.rs — tree-sitter AST analysis for TypeScript/TSX cheating patterns.
//
// Complements parsers::typescript (regex-based line scanner) with structural
// analysis that requires understanding the AST:
//
//   STRANGE-012: SHA-256 call on a crypto API (should be BLAKE3)
//   STRANGE-013: Math.random() / Date.now() in non-test files (determinism violation)
//   STRANGE-014: vi.mock / jest.mock in non-test files (test double outside test)
//   STRANGE-015: engine_source literal 'synthetic' (receipt forgery signal)
//   STRANGE-016: Hardcoded 64-char hex string in test assertions (oracle hash)
//   STRANGE-017: crypto.subtle.digest('SHA-256', ...) call (should use BLAKE3)
//   STRANGE-018: console.log in production server route (data leak surface)
//
// Each observation is fed into rules::typescript_ast::evaluate() which emits
// AntiLlmDiagnostic structs for the LSP diagnostics layer.

use std::sync::OnceLock;

use regex::Regex;
use tree_sitter::{Node, Parser};

use crate::observations::Observation;

// ── Compiled-once patterns ────────────────────────────────────────────────────

fn hex64_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // 64 lowercase hex chars in a string literal — potential hardcoded oracle hash
    RE.get_or_init(|| Regex::new(r#"["'][0-9a-f]{64}["']"#).unwrap())
}

fn sha256_digest_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // crypto.subtle.digest('SHA-256', ...) — must migrate to BLAKE3
    RE.get_or_init(|| Regex::new(r#"digest\s*\(\s*['"]SHA-256['"]\s*,"#).unwrap())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn node_text<'a>(node: Node<'a>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or_default()
}

fn is_test_file(filepath: &str) -> bool {
    filepath.contains(".test.")
        || filepath.contains(".spec.")
        || filepath.contains("/test/")
        || filepath.contains("/tests/")
        || filepath.contains("/__tests__/")
        || filepath.contains("/fixtures/")
}

fn is_server_route(filepath: &str) -> bool {
    filepath.contains("/server/api/") || filepath.contains("/server/routes/")
}

// ── AST traversal ─────────────────────────────────────────────────────────────

fn traverse(node: Node, source: &[u8], filepath: &str, is_test: bool, obs: &mut Vec<Observation>) {
    let kind = node.kind();
    let range = node.range();
    let text = node_text(node, source);

    // STRANGE-012 / STRANGE-017: SHA-256 usage — any form
    // Catches: createHash('sha256'), digest('SHA-256',...), crypto.createHash
    if kind == "string"
        && (text.eq_ignore_ascii_case("'sha256'")
            || text.eq_ignore_ascii_case("\"sha256\"")
            || text.eq_ignore_ascii_case("'sha-256'")
            || text.eq_ignore_ascii_case("\"sha-256\""))
    {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: range.start_byte,
            end_byte: range.end_byte,
            line: range.start_point.row + 1,
            column: range.start_point.column + 1,
            kind: "ast_ts_sha256".to_string(),
            construct: "sha256_literal".to_string(),
            context: text.chars().take(120).collect(),
            message: format!("SHA-256 algorithm literal '{}' — must migrate to BLAKE3", text),
        });
    }

    // STRANGE-013: Math.random() / Date.now() in non-test files
    if !is_test && kind == "call_expression" {
        let callee =
            node.child_by_field_name("function").map(|n| node_text(n, source)).unwrap_or_default();
        if callee == "Math.random" || callee == "Date.now" || callee == "new Date" {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: range.start_byte,
                end_byte: range.end_byte,
                line: range.start_point.row + 1,
                column: range.start_point.column + 1,
                kind: "ast_ts_nondeterminism".to_string(),
                construct: callee.to_string(),
                context: text.chars().take(120).collect(),
                message: format!(
                    "Non-deterministic call '{}' in production code — breaks replay law",
                    callee
                ),
            });
        }
    }

    // STRANGE-014: vi.mock / jest.mock outside test files
    if !is_test && kind == "call_expression" {
        let callee =
            node.child_by_field_name("function").map(|n| node_text(n, source)).unwrap_or_default();
        if callee == "vi.mock"
            || callee == "jest.mock"
            || callee.ends_with(".mock")
            || callee.ends_with(".spyOn")
        {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: range.start_byte,
                end_byte: range.end_byte,
                line: range.start_point.row + 1,
                column: range.start_point.column + 1,
                kind: "ast_ts_mock_leak".to_string(),
                construct: callee.to_string(),
                context: text.chars().take(120).collect(),
                message: format!(
                    "Test double '{}' found outside test file — mocks in production code break observability",
                    callee
                ),
            });
        }
    }

    // STRANGE-016: Hardcoded 64-char hex oracle hash in test assertions
    if is_test && kind == "string" && hex64_re().is_match(text) {
        // Only flag inside expect() / toBe() / toEqual() call contexts
        let in_assertion = {
            let mut p = node.parent();
            let mut found = false;
            for _ in 0..5 {
                if let Some(parent) = p {
                    let pt = node_text(parent, source);
                    if pt.contains("expect(") || pt.contains(".toBe(") || pt.contains(".toEqual(") {
                        found = true;
                        break;
                    }
                    p = parent.parent();
                } else {
                    break;
                }
            }
            found
        };
        if in_assertion {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: range.start_byte,
                end_byte: range.end_byte,
                line: range.start_point.row + 1,
                column: range.start_point.column + 1,
                kind: "ast_ts_oracle_hash".to_string(),
                construct: "hardcoded_64hex".to_string(),
                context: text.chars().take(120).collect(),
                message: "Hardcoded 64-char hex in test assertion is an oracle hash — compute it dynamically with BLAKE3".to_string(),
            });
        }
    }

    // STRANGE-017: crypto.subtle.digest('SHA-256', ...) call
    if kind == "call_expression" {
        let callee =
            node.child_by_field_name("function").map(|n| node_text(n, source)).unwrap_or_default();
        if callee.contains("digest") {
            let args_text = node
                .child_by_field_name("arguments")
                .map(|n| node_text(n, source))
                .unwrap_or_default();
            if sha256_digest_re().is_match(args_text) {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: range.start_byte,
                    end_byte: range.end_byte,
                    line: range.start_point.row + 1,
                    column: range.start_point.column + 1,
                    kind: "ast_ts_sha256_digest".to_string(),
                    construct: "crypto.subtle.digest:SHA-256".to_string(),
                    context: text.chars().take(120).collect(),
                    message: "crypto.subtle.digest('SHA-256') call — WebCrypto SHA-256 must be replaced with @noble/hashes/blake3".to_string(),
                });
            }
        }
    }

    // STRANGE-018: console.log in server routes (data leak surface)
    if is_server_route(filepath) && kind == "call_expression" {
        let callee =
            node.child_by_field_name("function").map(|n| node_text(n, source)).unwrap_or_default();
        if callee == "console.log" || callee == "console.error" || callee == "console.warn" {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: range.start_byte,
                end_byte: range.end_byte,
                line: range.start_point.row + 1,
                column: range.start_point.column + 1,
                kind: "ast_ts_console_leak".to_string(),
                construct: callee.to_string(),
                context: text.chars().take(120).collect(),
                message: format!(
                    "{} in server route may leak PII or internal state — use structured logging",
                    callee
                ),
            });
        }
    }

    // Recurse
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            traverse(cursor.node(), source, filepath, is_test, obs);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn parse_typescript_ast(filepath: &str, content: &str) -> Vec<Observation> {
    let mut obs = Vec::new();
    let mut parser = Parser::new();

    // tree-sitter-typescript exposes both TypeScript and TSX grammars.
    // Use the TSX grammar for all .ts/.tsx files — it is a strict superset.
    let lang = tree_sitter_typescript::LANGUAGE_TSX.into();
    if parser.set_language(&lang).is_err() {
        return obs;
    }

    let Some(tree) = parser.parse(content, None) else {
        return obs;
    };

    let is_test = is_test_file(filepath);
    traverse(tree.root_node(), content.as_bytes(), filepath, is_test, &mut obs);
    obs
}
