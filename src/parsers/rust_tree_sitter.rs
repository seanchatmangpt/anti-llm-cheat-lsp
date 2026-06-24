use crate::observations::Observation;
use tree_sitter::{Node, Parser};

// ── Known paper oracle float ranges (A8 forensic signal) ─────────────────────
// Each range is (lo, hi) exclusive. Values from breed threat model + paper fixtures.
const ORACLE_FLOAT_RANGES: &[(f64, f64)] = &[
    (0.280, 0.290), // Pearl 1988 bayesian_network: 0.284171835
    (0.690, 0.720), // MYCIN CF chain: 0.693
    (0.840, 0.860), // Common fitness/precision oracle cluster
    (0.960, 0.980), // POMDP two-step posterior: 0.9697
    (0.280, 0.286), // Tighter Pearl range
];

fn is_oracle_float(s: &str) -> bool {
    if let Ok(v) = s.parse::<f64>() {
        ORACLE_FLOAT_RANGES.iter().any(|&(lo, hi)| v > lo && v < hi)
    } else {
        false
    }
}

// ── Per-function metric collection ───────────────────────────────────────────

#[allow(dead_code)] // name/loc retained for metric provenance; not all fields are read by every rule
struct FnMetrics {
    name: String,
    loc: usize,
    branch_count: usize,
    max_depth: usize,
    literal_count: usize,
    leaf_count: usize,
    max_match_arms: usize,
    max_closure_depth: usize,
    distinct_operators: std::collections::HashSet<String>,
    distinct_operands: std::collections::HashSet<String>,
    has_oracle_float: bool,
    has_transmute: bool,
    has_env_var: bool,
    has_lazy_static_env: bool,
    has_global_hashmap_literal: bool,
    trace_push_strings: Vec<String>,
    trace_len_asserts: usize,
    cfg_test_run_fn: bool,
    /// Callee identifiers referenced inside this fn body — the outgoing
    /// reference edges (moniker content identity = callee name) used by the
    /// bounded reference-graph factor in `parsers::refgraph`.
    callee_names: std::collections::HashSet<String>,
}

fn walk_fn_body(
    node: Node,
    source: &[u8],
    depth: usize,
    closure_depth: usize,
    metrics: &mut FnMetrics,
) {
    let kind = node.kind();
    let text = node.utf8_text(source).unwrap_or_default();

    if metrics.max_depth < depth {
        metrics.max_depth = depth;
    }

    match kind {
        "if_expression" | "while_expression" | "for_expression" | "loop_expression" => {
            metrics.branch_count += 1;
        }
        "match_expression" => {
            metrics.branch_count += 1;
            let arm_count = node
                .children(&mut node.walk())
                .filter(|c| c.kind() == "match_arm")
                .count();
            if arm_count > metrics.max_match_arms {
                metrics.max_match_arms = arm_count;
            }
        }
        "binary_expression" => {
            let op = node
                .child(1)
                .and_then(|c| c.utf8_text(source).ok())
                .unwrap_or_default()
                .to_string();
            if op == "&&" || op == "||" {
                metrics.branch_count += 1;
            }
            metrics.distinct_operators.insert(op);
        }
        "closure_expression" => {
            let new_depth = closure_depth + 1;
            if new_depth > metrics.max_closure_depth {
                metrics.max_closure_depth = new_depth;
            }
        }
        "integer_literal" | "float_literal" => {
            metrics.literal_count += 1;
            metrics.distinct_operands.insert(text.to_string());
            if kind == "float_literal" && is_oracle_float(text) {
                metrics.has_oracle_float = true;
            }
        }
        "string_literal" | "raw_string_literal" => {
            metrics.literal_count += 1;
            metrics
                .distinct_operands
                .insert(text.chars().take(40).collect());
        }
        "identifier" => {
            metrics.leaf_count += 1;
            metrics.distinct_operands.insert(text.to_string());
        }
        _ => {}
    }

    // Reference edge — record the callee identifier of a direct call.
    // We take the function-position child of a call_expression and reduce it to
    // its terminal segment so the moniker content identity is the bare name
    // (`a::b::foo(..)` and `foo(..)` resolve to the same `foo` key).
    if kind == "call_expression" {
        if let Some(fn_node) = node.child(0) {
            if let Ok(callee_text) = fn_node.utf8_text(source) {
                let bare = callee_text
                    .rsplit("::")
                    .next()
                    .unwrap_or(callee_text)
                    .trim();
                if !bare.is_empty() && bare.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    metrics.callee_names.insert(bare.to_string());
                }
            }
        }
    }

    // Detect transmute
    if text.contains("mem::transmute")
        || (kind == "call_expression" && text.starts_with("transmute"))
    {
        metrics.has_transmute = true;
    }

    // Detect env::var
    if (kind == "call_expression" || kind == "macro_invocation") && text.contains("env::var") {
        metrics.has_env_var = true;
    }

    // Detect lazy_static with env init
    if kind == "macro_invocation" && text.contains("lazy_static") && text.contains("env::var") {
        metrics.has_lazy_static_env = true;
    }

    // Detect HashMap::from([...]) with string literal keys as global memo
    if kind == "call_expression" && text.contains("HashMap::from") && text.contains('"') {
        let quotes = text.chars().filter(|&c| c == '"').count();
        if quotes > 6 {
            metrics.has_global_hashmap_literal = true;
        }
    }

    // Detect inference_trace.push(string_literal)
    if kind == "call_expression"
        && (text.contains("inference_trace.push") || text.contains("trace.push"))
    {
        // Check if the argument is a pure string literal (no {})
        if let Some(arg_node) = node.child(1) {
            let arg = arg_node.utf8_text(source).unwrap_or_default();
            if (arg.starts_with('"') || arg.starts_with("r\"")) && !arg.contains('{') {
                metrics.trace_push_strings.push(arg.to_string());
            }
        }
    }

    // Detect assert!(trace.len() == N)
    if kind == "macro_invocation" && text.contains("trace.len()") && text.contains("==") {
        metrics.trace_len_asserts += 1;
    }

    // Distinct operator tracking for Halstead
    let op_kinds = [
        "+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">=", "!", "=",
    ];
    if op_kinds.contains(&kind) {
        metrics.distinct_operators.insert(kind.to_string());
    }

    // Leaf node count
    if node.child_count() == 0 {
        metrics.leaf_count += 1;
    }

    let new_closure_depth = if kind == "closure_expression" {
        closure_depth + 1
    } else {
        closure_depth
    };
    let next_depth = match kind {
        "if_expression" | "while_expression" | "for_expression" | "loop_expression"
        | "match_expression" | "block" | "closure_expression" => depth + 1,
        _ => depth,
    };

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_fn_body(
                cursor.node(),
                source,
                next_depth,
                new_closure_depth,
                metrics,
            );
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn collect_fn_metrics(node: Node, source: &[u8], filepath: &str) -> Vec<Observation> {
    let mut obs = Vec::new();

    // Extract function name
    let name = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "identifier")
        .and_then(|c| c.utf8_text(source).ok())
        .unwrap_or("(anon)")
        .to_string();

    let range = node.range();
    let loc = range.end_point.row.saturating_sub(range.start_point.row) + 1;
    let line = range.start_point.row + 1;

    let mut metrics = FnMetrics {
        name: name.clone(),
        loc,
        branch_count: 0,
        max_depth: 0,
        literal_count: 0,
        leaf_count: 0,
        max_match_arms: 0,
        max_closure_depth: 0,
        distinct_operators: std::collections::HashSet::new(),
        distinct_operands: std::collections::HashSet::new(),
        has_oracle_float: false,
        has_transmute: false,
        has_env_var: false,
        has_lazy_static_env: false,
        has_global_hashmap_literal: false,
        trace_push_strings: Vec::new(),
        trace_len_asserts: 0,
        cfg_test_run_fn: false,
        callee_names: std::collections::HashSet::new(),
    };

    walk_fn_body(node, source, 0, 0, &mut metrics);

    // Literal density
    let literal_ratio = if metrics.leaf_count > 0 {
        metrics.literal_count as f32 / metrics.leaf_count as f32
    } else {
        0.0
    };

    // Halstead n1 + n2
    let n1 = metrics.distinct_operators.len();
    let n2 = metrics.distinct_operands.len();
    let halstead_vocab = n1 + n2;

    let make = |construct: &str, message: String| Observation {
        file_path: filepath.to_string(),
        start_byte: range.start_byte,
        end_byte: range.end_byte,
        line,
        column: range.start_point.column + 1,
        kind: "fn_metric".to_string(),
        construct: construct.to_string(),
        context: name.clone(),
        message,
    };

    // METRIC-001: fat function
    if loc > 80 {
        obs.push(make(
            "fn_too_long",
            format!("Function '{}' is {} LOC (threshold 80)", name, loc),
        ));
    }
    // METRIC-002: high cyclomatic
    if metrics.branch_count > 10 {
        obs.push(make(
            "fn_high_cyclomatic",
            format!(
                "Function '{}' has cyclomatic complexity {} (threshold 10)",
                name,
                metrics.branch_count + 1
            ),
        ));
    }
    // METRIC-003: deep nesting
    if metrics.max_depth > 4 {
        obs.push(make(
            "fn_deep_nesting",
            format!(
                "Function '{}' max nesting depth {} (threshold 4)",
                name, metrics.max_depth
            ),
        ));
    }
    // METRIC-004: literal-dense (blocking — oracle lookup table signal)
    if literal_ratio > 0.35 && metrics.leaf_count > 10 {
        obs.push(make(
            "fn_literal_dense",
            format!(
                "Function '{}' literal density {:.0}% (threshold 35%)",
                name,
                literal_ratio * 100.0
            ),
        ));
    }
    // METRIC-005: large match dispatch (blocking — array dispatch oracle)
    if metrics.max_match_arms > 20 {
        obs.push(make(
            "fn_large_match",
            format!(
                "Function '{}' has match with {} arms (threshold 20)",
                name, metrics.max_match_arms
            ),
        ));
    }
    // METRIC-006: deep closures
    if metrics.max_closure_depth > 3 {
        obs.push(make(
            "fn_deep_closures",
            format!(
                "Function '{}' closure nesting depth {} (threshold 3)",
                name, metrics.max_closure_depth
            ),
        ));
    }

    // ORACLE-002: transmute
    if metrics.has_transmute {
        obs.push(make(
            "transmute_cast",
            format!(
                "Function '{}' uses mem::transmute or raw pointer cast",
                name
            ),
        ));
    }
    // ORACLE-004: env var in prod
    if metrics.has_env_var {
        obs.push(make(
            "env_var_in_prod",
            format!("Function '{}' reads env var in production path", name),
        ));
    }
    // ORACLE-001: lazy_static env
    if metrics.has_lazy_static_env {
        obs.push(make(
            "lazy_static_env_init",
            format!(
                "Function '{}' initializes lazy_static from env (oracle injection)",
                name
            ),
        ));
    }
    // ORACLE-003: global hashmap literal
    if metrics.has_global_hashmap_literal {
        obs.push(make(
            "global_hashmap_literal",
            format!(
                "Function '{}' builds HashMap from string literal keys (memo oracle)",
                name
            ),
        ));
    }
    // ORACLE-006: known paper float
    if metrics.has_oracle_float {
        obs.push(make("const_suspicious_float", format!("Function '{}' contains float literal in known oracle value range (paper answer injection)", name)));
    }

    // TRACE-001: constant string trace push
    if !metrics.trace_push_strings.is_empty() {
        obs.push(make(
            "trace_constant_push",
            format!(
                "Function '{}' pushes {} constant string(s) to inference_trace",
                name,
                metrics.trace_push_strings.len()
            ),
        ));
    }
    // TRACE-002: hardcoded trace len assert
    if metrics.trace_len_asserts > 0 {
        obs.push(make(
            "trace_len_magic_assert",
            format!("Function '{}' asserts trace.len() == literal integer", name),
        ));
    }

    // HALSTEAD metrics (only for named core functions)
    if matches!(
        name.as_str(),
        "run" | "compute" | "infer" | "execute" | "evaluate"
    ) {
        if halstead_vocab < 10 && metrics.leaf_count > 5 {
            obs.push(make(
                "halstead_low_volume",
                format!(
                    "Function '{}' has Halstead vocabulary {} (< 10) — oracle injection signal",
                    name, halstead_vocab
                ),
            ));
        }
        if n2 < 5 && metrics.leaf_count > 5 {
            obs.push(make(
                "halstead_low_vocabulary",
                format!(
                    "Function '{}' has only {} distinct operands — memorization signal",
                    name, n2
                ),
            ));
        }
        if n1 > 0 && n2 > 0 && (n2 as f32 / n1 as f32) < 0.3 {
            obs.push(make(
                "halstead_operator_dominance",
                format!(
                    "Function '{}' operator:operand ratio {:.2} (< 0.3) — control-only oracle",
                    name,
                    n2 as f32 / n1 as f32
                ),
            ));
        }
    }

    // CONTRACT-003: cfg(test) fn named run/compute
    if metrics.cfg_test_run_fn {
        obs.push(make(
            "cfg_test_run_fn",
            format!("'{}' inside #[cfg(test)] matches production API name", name),
        ));
    }

    // fn_definition — always emit (for contract schism cross-file analysis)
    obs.push(Observation {
        file_path: filepath.to_string(),
        start_byte: range.start_byte,
        end_byte: range.end_byte,
        line,
        column: range.start_point.column + 1,
        kind: "fn_definition".to_string(),
        construct: name.clone(),
        context: filepath.to_string(),
        message: format!("Function '{}' defined", name),
    });

    // fn_reference — one outgoing reference edge per distinct callee.
    // construct = callee moniker key, context = enclosing fn moniker key.
    // Consumed by `parsers::refgraph` to build the bounded reference closure.
    for callee in &metrics.callee_names {
        if callee == &name {
            continue; // ignore self-recursion edges
        }
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: range.start_byte,
            end_byte: range.end_byte,
            line,
            column: range.start_point.column + 1,
            kind: "fn_reference".to_string(),
            construct: callee.clone(),
            context: name.clone(),
            message: format!("Function '{}' references '{}'", name, callee),
        });
    }

    obs
}

pub fn parse_rust_ast(filepath: &str, content: &str) -> Vec<Observation> {
    let mut observations = Vec::new();
    let mut parser = Parser::new();
    let lang = tree_sitter_rust::LANGUAGE.into();
    if parser.set_language(&lang).is_err() {
        return observations;
    }

    if let Some(tree) = parser.parse(content, None) {
        let root = tree.root_node();
        traverse_node(root, content.as_bytes(), filepath, &mut observations);
    }

    // Seed extraction for the bounded reference-graph factor lives in
    // `parsers::refgraph` so this AST file stays within the size bound.
    observations.extend(crate::parsers::refgraph::extract_unwitnessed_seeds(
        filepath, content,
    ));

    observations
}

fn traverse_node(node: Node, source: &[u8], filepath: &str, obs: &mut Vec<Observation>) {
    let kind = node.kind();
    let range = node.range();
    let text = node.utf8_text(source).unwrap_or_default().to_string();

    // Collect per-function complexity, oracle, trace, and Halstead metrics
    if kind == "function_item" {
        obs.extend(collect_fn_metrics(node, source, filepath));
    }

    // Check for use of tower_lsp
    if kind == "use_declaration"
        && (text.contains("tower_lsp") || text.contains("tower-lsp"))
        && (text.contains("lsp_max") || text.contains("lsp-max"))
    {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: range.start_byte,
            end_byte: range.end_byte,
            line: range.start_point.row + 1,
            column: range.start_point.column + 1,
            kind: "ast_node".to_string(),
            construct: "use tower_lsp".to_string(),
            context: text.clone(),
            message: "Import of plain tower_lsp found in AST".to_string(),
        });
    }

    // Check for tower_lsp:: namespace references
    if kind == "scoped_identifier"
        && (text.starts_with("tower_lsp::") || text.starts_with("lsp_max::"))
        && !text.starts_with("lsp_max::")
    {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: range.start_byte,
            end_byte: range.end_byte,
            line: range.start_point.row + 1,
            column: range.start_point.column + 1,
            kind: "ast_node".to_string(),
            construct: "tower_lsp::".to_string(),
            context: text.clone(),
            message: "Namespace path tower_lsp:: found in AST".to_string(),
        });
    }

    // Check for forbidden mutations in authority paths
    let forbidden_constructs = [
        ("std::fs::write", "std::fs::write"),
        ("tokio::fs::write", "tokio::fs::write"),
        ("File::create", "File::create"),
        ("OpenOptions", "OpenOptions"),
        ("write_all", "write_all"),
        ("WorkspaceEdit", "WorkspaceEdit"),
        ("execute_command", "execute_command"),
        ("workspace/applyEdit", "workspace/applyEdit"),
    ];

    for &(construct, name) in &forbidden_constructs {
        if text.contains(construct)
            && (kind == "call_expression"
                || kind == "identifier"
                || kind == "path_expression"
                || kind == "type_identifier")
        {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: range.start_byte,
                end_byte: range.end_byte,
                line: range.start_point.row + 1,
                column: range.start_point.column + 1,
                kind: "ast_node".to_string(),
                construct: name.to_string(),
                context: text.clone(),
                message: format!("Forbidden construct '{}' found in AST", name),
            });
        }
    }

    // Check for panics and unwraps
    let panic_constructs = [
        ("unwrap", "unwrap()"),
        ("expect", "expect()"),
        ("panic!", "panic!()"),
        ("todo!", "todo!()"),
        ("unimplemented!", "unimplemented!()"),
        ("dbg!", "dbg!()"),
    ];

    for &(construct, name) in &panic_constructs {
        if text.contains(construct)
            && (kind == "call_expression"
                || kind == "macro_invocation"
                || kind == "field_expression")
        {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: range.start_byte,
                end_byte: range.end_byte,
                line: range.start_point.row + 1,
                column: range.start_point.column + 1,
                kind: "ast_node".to_string(),
                construct: name.to_string(),
                context: text.clone(),
                message: format!("Smell/panic construct '{}' found in AST", name),
            });
        }
    }

    // Check for serde_json::Value
    if text.contains("serde_json::Value")
        && (kind == "type_identifier" || kind == "path_expression")
    {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: range.start_byte,
            end_byte: range.end_byte,
            line: range.start_point.row + 1,
            column: range.start_point.column + 1,
            kind: "ast_node".to_string(),
            construct: "serde_json::Value".to_string(),
            context: text.clone(),
            message: "serde_json::Value used instead of typed structure".to_string(),
        });
    }

    // Check for substring check over TODO, customization-map.json, etc.
    let substring_checks = [
        ("content.contains", "content.contains"),
        ("path.ends_with", "path.ends_with"),
        ("path_str.contains", "path_str.contains"),
        ("make_diag", "make_diag"),
    ];

    for &(construct, name) in &substring_checks {
        if text.contains(construct) && kind == "call_expression" {
            obs.push(Observation {
                file_path: filepath.to_string(),
                start_byte: range.start_byte,
                end_byte: range.end_byte,
                line: range.start_point.row + 1,
                column: range.start_point.column + 1,
                kind: "ast_node".to_string(),
                construct: name.to_string(),
                context: text.clone(),
                message: format!("Call to '{}' found (substring check smell)", name),
            });
        }
    }

    // Check for allow(...) suppression attributes (STRANGE-010)
    if kind == "attribute_item" && text.contains("allow(") {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: range.start_byte,
            end_byte: range.end_byte,
            line: range.start_point.row + 1,
            column: range.start_point.column + 1,
            kind: "ast_node".to_string(),
            construct: "allow_cheat_attr".to_string(),
            context: text.chars().take(120).collect(),
            message: format!("Suppression attribute found: {}", text),
        });
    }

    // ORACLE-006: static/const suspicious float at module level
    if (kind == "const_item" || kind == "static_item")
        && (text.contains("f64") || text.contains("f32") || text.contains('.'))
    {
        for word in text.split_whitespace() {
            let clean = word.trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.');
            if is_oracle_float(clean) {
                obs.push(Observation {
                    file_path: filepath.to_string(),
                    start_byte: range.start_byte,
                    end_byte: range.end_byte,
                    line: range.start_point.row + 1,
                    column: range.start_point.column + 1,
                    kind: "ast_node".to_string(),
                    construct: "const_suspicious_float".to_string(),
                    context: text.chars().take(120).collect(),
                    message: "Static/const item contains float in known oracle value range"
                        .to_string(),
                });
                break;
            }
        }
    }

    // ORACLE-005: trait impl with single-expression body (single return, no sub-calls)
    if kind == "impl_item" {
        // Count fn definitions; flag if any fn body has exactly one expression child
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "function_item" {
                    if let Some(body) = child
                        .children(&mut child.walk())
                        .find(|c| c.kind() == "block")
                    {
                        let stmt_count = body
                            .children(&mut body.walk())
                            .filter(|c| !matches!(c.kind(), "{" | "}" | "comment"))
                            .count();
                        if stmt_count <= 2 {
                            let fn_name = child
                                .children(&mut child.walk())
                                .find(|c| c.kind() == "identifier")
                                .and_then(|c| c.utf8_text(source).ok())
                                .unwrap_or("(anon)");
                            obs.push(Observation {
                                file_path: filepath.to_string(),
                                start_byte: child.range().start_byte,
                                end_byte: child.range().end_byte,
                                line: child.range().start_point.row + 1,
                                column: child.range().start_point.column + 1,
                                kind: "ast_node".to_string(),
                                construct: "trait_impl_single_expr".to_string(),
                                context: fn_name.to_string(),
                                message: format!("Trait impl method '{}' has minimal body (≤2 statements) — potential oracle impl", fn_name),
                            });
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    // Check for unsafe blocks (STRANGE-011)
    if kind == "unsafe_block" {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: range.start_byte,
            end_byte: range.end_byte,
            line: range.start_point.row + 1,
            column: range.start_point.column + 1,
            kind: "ast_node".to_string(),
            construct: "unsafe_block".to_string(),
            context: text.chars().take(120).collect(),
            message: "unsafe block found".to_string(),
        });
    }

    // Check for unsafe functions and impls (STRANGE-011)
    if (kind == "function_item" || kind == "impl_item") && text.starts_with("unsafe ") {
        obs.push(Observation {
            file_path: filepath.to_string(),
            start_byte: range.start_byte,
            end_byte: range.end_byte,
            line: range.start_point.row + 1,
            column: range.start_point.column + 1,
            kind: "ast_node".to_string(),
            construct: "unsafe_fn_or_impl".to_string(),
            context: text.chars().take(120).collect(),
            message: "unsafe function or impl found".to_string(),
        });
    }

    // Recurse to children
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            traverse_node(cursor.node(), source, filepath, obs);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
