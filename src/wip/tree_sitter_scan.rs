use std::path::Path;

use tree_sitter::{Language, Node, Parser, Tree};

use super::{
    model::{SourceFinding, Standing, WipKind},
    source::finding_id,
};

/// Tree-sitter languages admitted by the WIP source scanner.
pub const SUPPORTED_TREE_SITTER_LANGUAGES: &[&str] = &[
    "rust",
    "typescript",
    "tsx",
    "javascript",
    "python",
    "go",
    "java",
    "c",
    "cpp",
    "csharp",
    "bash",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntaxLanguage {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Bash,
}

const RUST: &[SyntaxLanguage] = &[SyntaxLanguage::Rust];
const TYPESCRIPT: &[SyntaxLanguage] = &[SyntaxLanguage::TypeScript];
const TSX: &[SyntaxLanguage] = &[SyntaxLanguage::Tsx];
const JAVASCRIPT: &[SyntaxLanguage] = &[SyntaxLanguage::JavaScript];
const PYTHON: &[SyntaxLanguage] = &[SyntaxLanguage::Python];
const GO: &[SyntaxLanguage] = &[SyntaxLanguage::Go];
const JAVA: &[SyntaxLanguage] = &[SyntaxLanguage::Java];
const C: &[SyntaxLanguage] = &[SyntaxLanguage::C];
const CPP: &[SyntaxLanguage] = &[SyntaxLanguage::Cpp];
const C_OR_CPP: &[SyntaxLanguage] = &[SyntaxLanguage::C, SyntaxLanguage::Cpp];
const CSHARP: &[SyntaxLanguage] = &[SyntaxLanguage::CSharp];
const BASH: &[SyntaxLanguage] = &[SyntaxLanguage::Bash];

impl SyntaxLanguage {
    const fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Bash => "bash",
        }
    }

    fn language(self) -> Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::Bash => tree_sitter_bash::LANGUAGE.into(),
        }
    }
}

/// Returns the admitted Tree-sitter language names.
pub const fn supported_tree_sitter_languages() -> &'static [&'static str] {
    SUPPORTED_TREE_SITTER_LANGUAGES
}

/// Returns whether a path has an admitted Tree-sitter grammar.
pub fn is_tree_sitter_path(path: &Path) -> bool {
    candidates_for_path(path).is_some()
}

/// Parse one supported source file and emit AST-grounded WIP findings.
///
/// `None` means the extension has no admitted grammar and the caller may use the
/// bounded text fallback. `Some` always means Tree-sitter owned the scan,
/// including parser-load or parser-execution failures.
pub(super) fn scan_tree_sitter_file(
    path: &Path,
    relative: &str,
    source: &str,
) -> Option<Vec<SourceFinding>> {
    let candidates = candidates_for_path(path)?;
    let mut best: Option<(SyntaxLanguage, Tree, usize)> = None;
    let mut load_failed = false;

    for &candidate in candidates {
        let mut parser = Parser::new();
        let language = candidate.language();
        if parser.set_language(&language).is_err() {
            load_failed = true;
            continue;
        }
        let Some(tree) = parser.parse(source, None) else {
            continue;
        };
        let score = parse_error_score(tree.root_node());
        let replace = best.as_ref().is_none_or(|(_, _, current_score)| score < *current_score);
        if replace {
            best = Some((candidate, tree, score));
        }
    }

    let Some((language, tree, _)) = best else {
        let marker =
            if load_failed { "TREE_SITTER_LANGUAGE_LOAD" } else { "TREE_SITTER_PARSE_UNAVAILABLE" };
        return Some(vec![SourceFinding {
            id: finding_id(relative, 1, marker),
            path: relative.to_string(),
            line: 1,
            marker: marker.to_string(),
            kind: source_kind(relative),
            standing: Standing::Unsupported,
            message: format!("tree-sitter parser unavailable for admitted extension: {relative}"),
        }]);
    };

    let mut findings = Vec::new();
    let root = tree.root_node();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        classify_node(language, node, relative, source, &mut findings);
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }

    findings.sort_by(|a, b| {
        a.path.cmp(&b.path).then(a.line.cmp(&b.line)).then(a.marker.cmp(&b.marker))
    });
    findings.dedup_by(|a, b| a.id == b.id);
    Some(findings)
}

fn classify_node(
    language: SyntaxLanguage,
    node: Node<'_>,
    relative: &str,
    source: &str,
    findings: &mut Vec<SourceFinding>,
) {
    let text = node_text(node, source);
    let kind = node.kind();

    if node.is_error() || node.is_missing() {
        push_finding(
            findings,
            relative,
            node,
            language,
            "syntax-error",
            source_kind(relative),
            Standing::BuildBroken,
            "Tree-sitter syntax error or missing syntax node",
            text,
        );
        return;
    }

    if is_comment_node(kind) {
        scan_comment(language, node, relative, text, findings);
        return;
    }

    if is_string_node(kind) && text.contains("file:///") {
        push_finding(
            findings,
            relative,
            node,
            language,
            "machine-local-replay",
            WipKind::Replay,
            Standing::PartialAlive,
            "machine-local replay pointer",
            text,
        );
    }

    match language {
        SyntaxLanguage::Rust => scan_rust_node(node, relative, text, findings),
        SyntaxLanguage::Python => scan_python_node(node, relative, text, source, findings),
        SyntaxLanguage::Go => scan_go_node(node, relative, text, findings),
        SyntaxLanguage::Java => scan_java_node(node, relative, text, findings),
        SyntaxLanguage::CSharp => scan_csharp_node(node, relative, text, findings),
        SyntaxLanguage::C | SyntaxLanguage::Cpp => {
            scan_c_family_node(language, node, relative, text, findings);
        }
        SyntaxLanguage::TypeScript | SyntaxLanguage::Tsx | SyntaxLanguage::JavaScript => {
            scan_js_family_node(language, node, relative, text, findings);
        }
        SyntaxLanguage::Bash => scan_bash_node(node, relative, text, findings),
    }
}

fn scan_comment(
    language: SyntaxLanguage,
    node: Node<'_>,
    relative: &str,
    text: &str,
    findings: &mut Vec<SourceFinding>,
) {
    let upper = text.to_ascii_uppercase();
    for marker in ["TODO", "FIXME", "XXX", "HACK", "TBD"] {
        if upper.contains(marker) {
            push_finding(
                findings,
                relative,
                node,
                language,
                marker,
                source_kind(relative),
                Standing::PartialAlive,
                "unfinished-work comment",
                text,
            );
        }
    }

    if text.contains("file:///") {
        push_finding(
            findings,
            relative,
            node,
            language,
            "machine-local-replay",
            WipKind::Replay,
            Standing::PartialAlive,
            "machine-local replay pointer",
            text,
        );
    }

    if let Some((marker, kind, standing, label)) = classify_comment_status(text) {
        push_finding(findings, relative, node, language, marker, kind, standing, label, text);
    }
}

fn scan_rust_node(node: Node<'_>, relative: &str, text: &str, findings: &mut Vec<SourceFinding>) {
    if node.kind() != "macro_invocation" {
        return;
    }
    let normalized = compact_whitespace(text).to_ascii_lowercase();
    let marker = if normalized.starts_with("todo!") || normalized.contains("::todo!") {
        Some(("todo!", "Rust todo! macro"))
    } else if normalized.starts_with("unimplemented!") || normalized.contains("::unimplemented!") {
        Some(("unimplemented!", "Rust unimplemented! macro"))
    } else if normalized.starts_with("panic!") && contains_unfinished_phrase(&normalized) {
        Some(("panic-unfinished", "Rust panic! unfinished implementation"))
    } else {
        None
    };

    if let Some((marker, label)) = marker {
        push_finding(
            findings,
            relative,
            node,
            SyntaxLanguage::Rust,
            marker,
            source_kind(relative),
            Standing::PartialAlive,
            label,
            text,
        );
    }
}

fn scan_python_node(
    node: Node<'_>,
    relative: &str,
    text: &str,
    source: &str,
    findings: &mut Vec<SourceFinding>,
) {
    if node.kind() == "function_definition" {
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            let children: Vec<_> = body.named_children(&mut cursor).collect();
            if children.len() == 1 {
                let child = children[0];
                let child_text = node_text(child, source).trim();
                if child.kind() == "pass_statement" {
                    push_finding(
                        findings,
                        relative,
                        child,
                        SyntaxLanguage::Python,
                        "pass-stub",
                        source_kind(relative),
                        Standing::PartialAlive,
                        "Python function body is only pass",
                        child_text,
                    );
                } else if child.kind() == "expression_statement" && child_text == "..." {
                    push_finding(
                        findings,
                        relative,
                        child,
                        SyntaxLanguage::Python,
                        "ellipsis-stub",
                        source_kind(relative),
                        Standing::PartialAlive,
                        "Python function body is only ellipsis",
                        child_text,
                    );
                }
            }
        }
    }

    if node.kind() == "raise_statement"
        && (text.contains("NotImplementedError") || contains_unfinished_phrase(text))
    {
        push_finding(
            findings,
            relative,
            node,
            SyntaxLanguage::Python,
            "raise-not-implemented",
            source_kind(relative),
            Standing::PartialAlive,
            "Python raises an unfinished-implementation sentinel",
            text,
        );
    }
}

fn scan_go_node(node: Node<'_>, relative: &str, text: &str, findings: &mut Vec<SourceFinding>) {
    if node.kind() == "call_expression" {
        let normalized = compact_whitespace(text).to_ascii_lowercase();
        if normalized.starts_with("panic(") && contains_unfinished_phrase(&normalized) {
            push_finding(
                findings,
                relative,
                node,
                SyntaxLanguage::Go,
                "panic-unfinished",
                source_kind(relative),
                Standing::PartialAlive,
                "Go panic marks unfinished implementation",
                text,
            );
        }
    }
}

fn scan_java_node(node: Node<'_>, relative: &str, text: &str, findings: &mut Vec<SourceFinding>) {
    if node.kind() == "throw_statement"
        && (text.contains("UnsupportedOperationException")
            || text.contains("NotImplementedException")
            || contains_unfinished_phrase(text))
    {
        push_finding(
            findings,
            relative,
            node,
            SyntaxLanguage::Java,
            "throw-not-implemented",
            source_kind(relative),
            Standing::PartialAlive,
            "Java throw marks unfinished implementation",
            text,
        );
    }
}

fn scan_csharp_node(node: Node<'_>, relative: &str, text: &str, findings: &mut Vec<SourceFinding>) {
    if matches!(node.kind(), "throw_statement" | "throw_expression")
        && (text.contains("NotImplementedException") || contains_unfinished_phrase(text))
    {
        push_finding(
            findings,
            relative,
            node,
            SyntaxLanguage::CSharp,
            "throw-not-implemented",
            source_kind(relative),
            Standing::PartialAlive,
            "C# throw marks unfinished implementation",
            text,
        );
    }
}

fn scan_c_family_node(
    language: SyntaxLanguage,
    node: Node<'_>,
    relative: &str,
    text: &str,
    findings: &mut Vec<SourceFinding>,
) {
    if matches!(node.kind(), "preproc_call" | "preproc_def") {
        let normalized = compact_whitespace(text).to_ascii_lowercase();
        if normalized.starts_with("#error") && contains_unfinished_phrase(&normalized) {
            push_finding(
                findings,
                relative,
                node,
                language,
                "preprocessor-unfinished",
                source_kind(relative),
                Standing::BuildBroken,
                "C/C++ preprocessor error marks unfinished implementation",
                text,
            );
        }
    }
}

fn scan_js_family_node(
    language: SyntaxLanguage,
    node: Node<'_>,
    relative: &str,
    text: &str,
    findings: &mut Vec<SourceFinding>,
) {
    if node.kind() == "throw_statement" && contains_unfinished_phrase(text) {
        push_finding(
            findings,
            relative,
            node,
            language,
            "throw-not-implemented",
            source_kind(relative),
            Standing::PartialAlive,
            "JavaScript/TypeScript throw marks unfinished implementation",
            text,
        );
    }
}

fn scan_bash_node(node: Node<'_>, relative: &str, text: &str, findings: &mut Vec<SourceFinding>) {
    if node.kind() == "command" {
        let normalized = compact_whitespace(text).to_ascii_lowercase();
        if normalized.starts_with("exit ") && contains_unfinished_phrase(&normalized) {
            push_finding(
                findings,
                relative,
                node,
                SyntaxLanguage::Bash,
                "exit-unfinished",
                source_kind(relative),
                Standing::PartialAlive,
                "Bash command marks unfinished implementation",
                text,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_finding(
    findings: &mut Vec<SourceFinding>,
    relative: &str,
    node: Node<'_>,
    language: SyntaxLanguage,
    marker: &str,
    kind: WipKind,
    standing: Standing,
    label: &str,
    evidence: &str,
) {
    let line = node.start_position().row + 1;
    let typed_marker = format!("tree-sitter:{}:{marker}", language.name());
    findings.push(SourceFinding {
        id: finding_id(relative, line, &typed_marker),
        path: relative.to_string(),
        line,
        marker: typed_marker,
        kind,
        standing,
        message: format!("tree-sitter/{} {label}: {}", language.name(), compact(evidence)),
    });
}

fn parse_error_score(root: Node<'_>) -> usize {
    let mut errors = 0;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            errors += 1;
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    errors
}

fn candidates_for_path(path: &Path) -> Option<&'static [SyntaxLanguage]> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "rs" => Some(RUST),
        "ts" | "mts" | "cts" => Some(TYPESCRIPT),
        "tsx" => Some(TSX),
        "js" | "mjs" | "cjs" | "jsx" => Some(JAVASCRIPT),
        "py" | "pyw" => Some(PYTHON),
        "go" => Some(GO),
        "java" => Some(JAVA),
        "c" => Some(C),
        "h" => Some(C_OR_CPP),
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => Some(CPP),
        "cs" => Some(CSHARP),
        "sh" | "bash" => Some(BASH),
        _ => None,
    }
}

fn source_kind(relative: &str) -> WipKind {
    if is_test_path(relative) {
        WipKind::Test
    } else {
        WipKind::Code
    }
}

fn is_test_path(relative: &str) -> bool {
    let normalized = relative.replace('\\', "/").to_ascii_lowercase();
    let file = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    normalized
        .split('/')
        .any(|part| matches!(part, "test" | "tests" | "spec" | "specs" | "__tests__"))
        || file.starts_with("test_")
        || file.ends_with("_test.go")
        || file.contains(".test.")
        || file.contains(".spec.")
        || file.ends_with("test.java")
        || file.ends_with("tests.java")
}

fn is_comment_node(kind: &str) -> bool {
    kind == "comment" || kind.ends_with("_comment")
}

fn is_string_node(kind: &str) -> bool {
    kind.contains("string") || matches!(kind, "raw_string_literal" | "char_literal")
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn contains_unfinished_phrase(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    normalized.contains("not implemented")
        || normalized.contains("notimplemented")
        || normalized.contains("unimplemented")
        || normalized.contains("todo")
        || normalized.contains("fixme")
        || normalized.contains("stub implementation")
}

fn classify_comment_status(text: &str) -> Option<(&'static str, WipKind, Standing, &'static str)> {
    let normalized = strip_comment_prefix(text).to_ascii_uppercase();
    let declares_status = normalized.starts_with("STATUS:")
        || normalized.starts_with("STANDING:")
        || normalized.starts_with("STATUS =")
        || normalized.starts_with("STANDING =");
    if !declares_status {
        return None;
    }

    if normalized.contains("BUILD_BROKEN") {
        Some(("BUILD_BROKEN", WipKind::Ci, Standing::BuildBroken, "declared broken build"))
    } else if normalized.contains("BLOCKED") {
        Some(("BLOCKED", WipKind::CrossRepoBlocker, Standing::Blocked, "declared blocker"))
    } else if normalized.contains("PARTIAL_ALIVE") {
        Some((
            "PARTIAL_ALIVE",
            WipKind::Evidence,
            Standing::PartialAlive,
            "declared partial standing",
        ))
    } else if normalized.contains("CANDIDATE") || normalized.contains("OPEN") {
        Some((
            "CANDIDATE_OR_OPEN",
            WipKind::Evidence,
            Standing::PartialAlive,
            "declared incomplete evidence state",
        ))
    } else {
        None
    }
}

fn strip_comment_prefix(text: &str) -> String {
    text.trim()
        .trim_start_matches('/')
        .trim_start_matches('*')
        .trim_start_matches('#')
        .trim_start_matches('-')
        .trim()
        .to_string()
}

fn compact(text: &str) -> String {
    let flattened = compact_whitespace(text);
    let mut chars = flattened.chars();
    let prefix: String = chars.by_ref().take(160).collect();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn compact_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
