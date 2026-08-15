use std::fs;

use anti_llm_cheat_lsp::wip::{
    scan_source_wip, supported_tree_sitter_languages, Standing, WipKind,
};

fn tempdir() -> tempfile::TempDir {
    match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => panic!("temporary directory failed: {error}"),
    }
}

#[test]
fn language_registry_is_polyglot() {
    let languages = supported_tree_sitter_languages();
    for expected in [
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
    ] {
        assert!(languages.contains(&expected));
    }
}

#[test]
fn rust_ast_ignores_marker_like_strings() {
    let dir = tempdir();
    assert!(fs::write(
        dir.path().join("lib.rs"),
        r#"pub fn message() -> &'static str { "TODO: not implemented" }
"#,
    )
    .is_ok());

    assert!(scan_source_wip(dir.path()).is_empty());
}

#[test]
fn rust_todo_macro_is_ast_wip() {
    let dir = tempdir();
    assert!(fs::write(
        dir.path().join("lib.rs"),
        "pub fn unfinished() { todo!(\"later\"); }\n",
    )
    .is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, WipKind::Code);
    assert!(findings[0].marker.contains("tree-sitter:rust:todo!"));
}

#[test]
fn python_pass_only_function_is_wip() {
    let dir = tempdir();
    assert!(fs::write(dir.path().join("service.py"), "def load():\n    pass\n").is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, WipKind::Code);
    assert!(findings[0].marker.contains("tree-sitter:python:pass-stub"));
}

#[test]
fn go_todo_panic_is_wip() {
    let dir = tempdir();
    assert!(fs::write(
        dir.path().join("worker.go"),
        "package worker\nfunc Run() { panic(\"TODO: wire backend\") }\n",
    )
    .is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert!(findings[0].marker.contains("tree-sitter:go:panic-unfinished"));
}

#[test]
fn typescript_not_implemented_throw_is_wip() {
    let dir = tempdir();
    assert!(fs::write(
        dir.path().join("client.ts"),
        "export function connect(): never { throw new Error(\"Not implemented\"); }\n",
    )
    .is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert!(findings[0]
        .marker
        .contains("tree-sitter:typescript:throw-not-implemented"));
}

#[test]
fn csharp_not_implemented_exception_is_wip() {
    let dir = tempdir();
    assert!(fs::write(
        dir.path().join("Service.cs"),
        "class Service { void Run() { throw new System.NotImplementedException(); } }\n",
    )
    .is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert!(findings[0]
        .marker
        .contains("tree-sitter:csharp:throw-not-implemented"));
}

#[test]
fn syntax_error_is_build_broken_wip() {
    let dir = tempdir();
    assert!(fs::write(dir.path().join("Broken.java"), "class Broken { void run( { }\n").is_ok());

    let findings = scan_source_wip(dir.path());
    assert!(findings
        .iter()
        .any(|finding| finding.standing == Standing::BuildBroken));
}

#[test]
fn test_file_stub_is_typed_as_test_wip() {
    let dir = tempdir();
    let tests = dir.path().join("tests");
    assert!(fs::create_dir_all(&tests).is_ok());
    assert!(fs::write(tests.join("test_api.py"), "def test_api():\n    pass\n").is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, WipKind::Test);
}

#[test]
fn cpp_header_uses_best_of_c_and_cpp_grammars() {
    let dir = tempdir();
    assert!(fs::write(
        dir.path().join("widget.h"),
        "class Widget { public: void run(); };\n",
    )
    .is_ok());

    assert!(scan_source_wip(dir.path()).is_empty());
}

#[test]
fn javascript_comment_marker_is_ast_wip() {
    let dir = tempdir();
    assert!(fs::write(
        dir.path().join("index.js"),
        "// FIXME: close transport\nexport const ready = false;\n",
    )
    .is_ok());

    let findings = scan_source_wip(dir.path());
    assert_eq!(findings.len(), 1);
    assert!(findings[0].marker.contains("tree-sitter:javascript:FIXME"));
}
