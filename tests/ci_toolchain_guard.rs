//! CI toolchain guard (hardening for PR #1: the dependabot bump of
//! `dtolnay/rust-toolchain` from `@1.82` to `@1.100`, replaced by a pin of the
//! msrv job to the `rust-toolchain.toml` channel).
//!
//! Observed failure that motivated this file: the bumped `msrv` job ran
//! `rustup toolchain install 1.100.0`, which returned HTTP 404 ("could not
//! download nonexistent rust version"). Independently, no stable toolchain can
//! build this crate: path dependencies use `#![feature(..)]` and edition 2024,
//! so the only toolchain that builds it is the pinned nightly in
//! `rust-toolchain.toml`.
//!
//! The guard below is a pure function over the real repository files. Tests
//! run it on the committed `ci.yml` / `rust-toolchain.toml` (must admit) and on
//! adversarial mutations of them (must refuse, with a typed violation). No test
//! doubles: the only collaborators are the real files and, for the witness
//! test, the real `rustup` resolving this checkout's toolchain-file override.
//!
//! Two independent readings of `ci.yml` are taken: a structural parse (jobs,
//! steps, `with:` inputs, block and flow style) and a lexical scan of every
//! non-comment line. Every `dtolnay/rust-toolchain@` reference the lexical scan
//! sees must also be seen by the parser; a mismatch is refused as
//! `UnparsedToolchainRef`, so a YAML form the parser does not model cannot
//! smuggle a toolchain past it.

#![allow(clippy::expect_used, clippy::unwrap_used)] // test code: a panic is the failure signal

use std::{path::Path, process::Command, time::Instant};

const ACTION: &str = "dtolnay/rust-toolchain@";

/// Typed refusal reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Violation {
    /// `rust-toolchain.toml` has no parseable `channel = "..."`.
    ChannelMissing,
    /// Channel is a floating name (`nightly`, `stable`, `beta`) or malformed.
    ChannelNotPinned(String),
    /// The workflow has no top-level `jobs:` mapping.
    JobsMissing,
    /// Number of jobs keyed `msrv` is not exactly one.
    MsrvJobCount(usize),
    /// The `msrv` job installs no toolchain via `dtolnay/rust-toolchain`.
    MsrvToolchainMissing,
    /// A toolchain installed by the `msrv` job differs from `rust-toolchain.toml`.
    MsrvToolchainDrift { job: String, pinned: String },
    /// The `msrv` job uses the action at a ref that is not a 40-hex commit.
    MsrvActionNotShaPinned { reference: String },
    /// A step uses `dtolnay/rust-toolchain@<numeric>`; the ref encodes a Rust
    /// version (bump bots move it, and a numeric stable cannot build the crate).
    NumericToolchainRef { job: String, reference: String },
    /// A `toolchain:` input is a numeric stable version.
    NumericToolchainInput { job: String, value: String },
    /// `dtolnay/rust-toolchain@master` (or a commit of it) without a
    /// `toolchain:` input.
    RefWithoutToolchain { job: String, reference: String },
    /// `RUSTUP_TOOLCHAIN` is set to something other than the pinned channel;
    /// it takes precedence over `rust-toolchain.toml`.
    ToolchainEnvOverride { value: String },
    /// A `run:` in the `msrv` job selects a toolchain by command
    /// (`cargo +<tc>`, `rustup default`, `rustup override`).
    ToolchainCommandOverride { line: String },
    /// The lexical scan saw more action references than the parser modelled.
    UnparsedToolchainRef { job: String, lexical: usize, parsed: usize },
}

#[derive(Debug, Default)]
struct Step {
    uses: Option<String>,
    toolchain: Option<String>,
}

#[derive(Debug)]
struct Job<'a> {
    key: String,
    steps: Vec<Step>,
    /// Non-comment body lines of the job, comments stripped (borrowed).
    lines: Vec<&'a str>,
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Drop a YAML comment: `#` at the start or preceded by whitespace.
fn strip_comment(line: &str) -> &str {
    let b = line.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b'#' && (i == 0 || b[i - 1].is_ascii_whitespace()) {
            return &line[..i];
        }
    }
    line
}

fn strip_value(v: &str) -> String {
    strip_comment(v).trim().trim_matches('"').trim_matches('\'').to_string()
}

/// Value of `key:` inside a flow mapping such as `{uses: a, with: {toolchain: b}}`.
fn flow_get(s: &str, key: &str) -> Option<String> {
    let pat = format!("{key}:");
    let mut from = 0;
    while let Some(off) = s[from..].find(&pat) {
        let i = from + off;
        let boundary = i == 0 || matches!(s.as_bytes()[i - 1], b'{' | b',' | b' ' | b'\t');
        if boundary {
            let rest = &s[i + pat.len()..];
            let end = rest.find([',', '}']).unwrap_or(rest.len());
            let v = strip_value(&rest[..end]);
            if !v.is_empty() && !v.starts_with('{') {
                return Some(v);
            }
        }
        from = i + pat.len();
    }
    None
}

/// Split `- rest` into `Some(rest)` for any amount of whitespace after the dash.
fn sequence_item(t: &str) -> Option<&str> {
    let rest = t.strip_prefix('-')?;
    if rest.is_empty() {
        return Some("");
    }
    rest.starts_with([' ', '\t']).then(|| rest.trim_start())
}

fn apply_inline(step: &mut Step, rest: &str) {
    if rest.starts_with('{') {
        if let Some(u) = flow_get(rest, "uses") {
            step.uses = Some(u);
        }
        if let Some(tc) = flow_get(rest, "toolchain") {
            step.toolchain = Some(tc);
        }
    } else if let Some(u) = rest.strip_prefix("uses:") {
        step.uses = Some(strip_value(u));
    }
}

/// Indentation-driven extractor for the GitHub Actions subset used in this
/// repository: block mappings, `-` step sequences (any spacing after the dash),
/// flow-style steps and `with:` blocks or flow mappings.
fn parse_jobs(yaml: &str) -> Result<Vec<Job<'_>>, Violation> {
    let lines: Vec<&str> = yaml.lines().collect();
    let start = lines
        .iter()
        .position(|l| strip_comment(l).trim_end() == "jobs:" && indent_of(l) == 0)
        .ok_or(Violation::JobsMissing)?;
    let mut jobs: Vec<Job> = Vec::new();
    let mut job_indent: Option<usize> = None;
    let mut step_indent: Option<usize> = None;
    let mut in_with = false;
    let mut with_indent = 0usize;
    for raw in &lines[start + 1..] {
        let t = strip_comment(raw).trim();
        if t.is_empty() {
            continue;
        }
        let ind = indent_of(raw);
        if ind == 0 {
            break; // next top-level key ends `jobs:`
        }
        let ji = *job_indent.get_or_insert(ind);
        if ind == ji {
            let key = t.trim_end_matches(':').to_string();
            jobs.push(Job { key, steps: Vec::new(), lines: Vec::new() });
            step_indent = None;
            in_with = false;
            continue;
        }
        let Some(job) = jobs.last_mut() else { continue };
        job.lines.push(t);
        if let Some(rest) = sequence_item(t) {
            if step_indent.is_none() || Some(ind) == step_indent {
                step_indent = Some(ind);
                let mut step = Step::default();
                apply_inline(&mut step, rest);
                job.steps.push(step);
                in_with = false;
                continue;
            }
        }
        if let (Some(si), Some(step)) = (step_indent, job.steps.last_mut()) {
            if ind <= si {
                in_with = false;
                continue;
            }
            if in_with && ind <= with_indent {
                in_with = false;
            }
            if let Some(w) = t.strip_prefix("with:") {
                let w = w.trim();
                if w.starts_with('{') {
                    if let Some(tc) = flow_get(w, "toolchain") {
                        step.toolchain = Some(tc);
                    }
                } else {
                    in_with = true;
                    with_indent = ind;
                }
            } else if let Some(u) = t.strip_prefix("uses:") {
                step.uses = Some(strip_value(u));
            } else if in_with {
                if let Some(v) = t.strip_prefix("toolchain:") {
                    step.toolchain = Some(strip_value(v));
                }
            }
        }
    }
    Ok(jobs)
}

fn pinned_channel(toolchain_toml: &str) -> Result<String, Violation> {
    let channel = toolchain_toml
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "channel").then(|| strip_value(v))
        })
        .ok_or(Violation::ChannelMissing)?;
    let dated_nightly = channel.strip_prefix("nightly-").is_some_and(|d| {
        let p: Vec<&str> = d.split('-').collect();
        p.len() == 3
            && p[0].len() == 4
            && p[1].len() == 2
            && p[2].len() == 2
            && p.iter().all(|x| x.chars().all(|c| c.is_ascii_digit()))
    });
    if dated_nightly || is_numeric(&channel) {
        Ok(channel)
    } else {
        Err(Violation::ChannelNotPinned(channel))
    }
}

fn is_numeric(r: &str) -> bool {
    let p: Vec<&str> = r.split('.').collect();
    (1..=3).contains(&p.len())
        && p.iter().all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
}

fn is_commit_sha(r: &str) -> bool {
    r.len() == 40 && r.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn is_delim(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\'' | '"' | ',' | '}' | ']')
}

/// Every `dtolnay/rust-toolchain@<ref>` in the given comment-stripped lines.
fn lexical_refs(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for l in lines {
        let mut s = *l;
        while let Some(i) = s.find(ACTION) {
            let rest = &s[i + ACTION.len()..];
            let end = rest.find(is_delim).unwrap_or(rest.len());
            out.push(rest[..end].to_string());
            s = &rest[end..];
        }
    }
    out
}

/// Every `toolchain:` input value (block or flow) in the given lines.
fn lexical_toolchain_inputs(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for l in lines {
        let mut from = 0;
        while let Some(off) = l[from..].find("toolchain:") {
            let i = from + off;
            let boundary = i == 0 || matches!(l.as_bytes()[i - 1], b'{' | b',' | b' ' | b'\t');
            let rest = &l[i + "toolchain:".len()..];
            if boundary {
                let end = rest.find([',', '}']).unwrap_or(rest.len());
                let v = strip_value(&rest[..end]);
                if !v.is_empty() {
                    out.push(v);
                }
            }
            from = i + "toolchain:".len();
        }
    }
    out
}

/// Every value assigned to `RUSTUP_TOOLCHAIN` anywhere in the workflow, in
/// `KEY: value` (env mapping, block or flow) or `KEY=value` (shell, `$GITHUB_ENV`).
fn rustup_toolchain_assignments(yaml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in yaml.lines() {
        let l = strip_comment(raw);
        let mut s = l;
        while let Some(i) = s.find("RUSTUP_TOOLCHAIN") {
            let rest = s[i + "RUSTUP_TOOLCHAIN".len()..].trim_start_matches(['"', '\'']);
            let rest = rest.trim_start();
            if let Some(v) = rest.strip_prefix(':').or_else(|| rest.strip_prefix('=')) {
                let v = v.trim_start();
                let end = v.find([',', '}', '>', ';', '&', '|']).unwrap_or(v.len());
                out.push(strip_value(&v[..end]));
            }
            s = &s[i + "RUSTUP_TOOLCHAIN".len()..];
        }
    }
    out
}

fn command_overrides(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| {
            l.contains("cargo +") || l.contains("rustup default") || l.contains("rustup override")
        })
        .map(|l| (*l).to_string())
        .collect()
}

/// Admission function: `Ok(())` admits, `Err` lists every violation.
fn check(ci_yaml: &str, toolchain_toml: &str) -> Result<(), Vec<Violation>> {
    let mut v = Vec::new();
    let pinned = match pinned_channel(toolchain_toml) {
        Ok(c) => Some(c),
        Err(e) => {
            v.push(e);
            None
        }
    };
    let jobs = match parse_jobs(ci_yaml) {
        Ok(j) => j,
        Err(e) => {
            v.push(e);
            return Err(v);
        }
    };
    for job in &jobs {
        let lexical = lexical_refs(&job.lines);
        let parsed: Vec<&Step> = job
            .steps
            .iter()
            .filter(|s| s.uses.as_deref().is_some_and(|u| u.starts_with(ACTION)))
            .collect();
        if lexical.len() != parsed.len() {
            v.push(Violation::UnparsedToolchainRef {
                job: job.key.clone(),
                lexical: lexical.len(),
                parsed: parsed.len(),
            });
        }
        for reference in &lexical {
            if is_numeric(reference) {
                v.push(Violation::NumericToolchainRef {
                    job: job.key.clone(),
                    reference: reference.clone(),
                });
            }
        }
        for value in lexical_toolchain_inputs(&job.lines) {
            if is_numeric(&value) {
                v.push(Violation::NumericToolchainInput { job: job.key.clone(), value });
            }
        }
        for step in &parsed {
            let reference = &step.uses.as_deref().expect("filtered")[ACTION.len()..];
            if (reference == "master" || is_commit_sha(reference)) && step.toolchain.is_none() {
                v.push(Violation::RefWithoutToolchain {
                    job: job.key.clone(),
                    reference: reference.to_string(),
                });
            }
        }
    }
    for value in rustup_toolchain_assignments(ci_yaml) {
        if pinned.as_deref() != Some(value.as_str()) {
            v.push(Violation::ToolchainEnvOverride { value });
        }
    }
    let msrv: Vec<&Job> = jobs.iter().filter(|j| j.key == "msrv").collect();
    if let [job] = msrv.as_slice() {
        let installs: Vec<(String, String)> = job
            .steps
            .iter()
            .filter_map(|s| {
                let r = s.uses.as_deref()?.strip_prefix(ACTION)?;
                Some((r.to_string(), s.toolchain.clone().unwrap_or_else(|| r.to_string())))
            })
            .collect();
        if installs.is_empty() {
            v.push(Violation::MsrvToolchainMissing);
        }
        for (reference, effective) in installs {
            if !is_commit_sha(&reference) {
                v.push(Violation::MsrvActionNotShaPinned { reference });
            }
            if let Some(p) = &pinned {
                if &effective != p {
                    v.push(Violation::MsrvToolchainDrift { job: effective, pinned: p.clone() });
                }
            }
        }
        for line in command_overrides(&job.lines) {
            v.push(Violation::ToolchainCommandOverride { line });
        }
    } else {
        v.push(Violation::MsrvJobCount(msrv.len()));
    }
    if v.is_empty() {
        Ok(())
    } else {
        Err(v)
    }
}

fn repo_file(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn real_ci() -> String {
    repo_file(".github/workflows/ci.yml")
}

fn real_toolchain() -> String {
    repo_file("rust-toolchain.toml")
}

const PINNED_SHA: &str = "02cb101ec7c40f2c49e1d9714d64511d8e1b74de";

/// The committed msrv install block, reconstructed from its parts.
fn msrv_block() -> String {
    format!(
        "      - uses: dtolnay/rust-toolchain@{PINNED_SHA} # master\n        with:\n          toolchain: nightly-2026-04-15\n"
    )
}

/// Replace the msrv job's toolchain install lines with `replacement`.
fn mutate_msrv(ci: &str, replacement: &str) -> String {
    let marker = msrv_block();
    assert!(ci.contains(&marker), "fixture drift: msrv install block not found in ci.yml");
    ci.replacen(&marker, replacement, 1)
}

/// Insert `extra` directly after the msrv install block.
fn append_to_msrv(ci: &str, extra: &str) -> String {
    let marker = msrv_block();
    mutate_msrv(ci, &format!("{marker}{extra}"))
}

fn refused(ci: &str) -> Vec<Violation> {
    check(ci, &real_toolchain()).expect_err("mutant must be refused")
}

// ── admit: the committed subject ─────────────────────────────────────────────

#[test]
fn committed_ci_and_toolchain_are_admitted() {
    assert_eq!(check(&real_ci(), &real_toolchain()), Ok(()));
}

#[test]
fn parser_sees_every_committed_job() {
    let ci = real_ci();
    let jobs = parse_jobs(&ci).expect("jobs");
    let keys: Vec<&str> = jobs.iter().map(|j| j.key.as_str()).collect();
    for k in ["fmt", "clippy", "test", "doc", "msrv", "deny", "typos", "coverage", "security-audit"]
    {
        assert!(keys.contains(&k), "job {k} missing from parse: {keys:?}");
    }
    let msrv = jobs.iter().find(|j| j.key == "msrv").expect("msrv");
    let tc = msrv.steps.iter().find_map(|s| s.toolchain.clone());
    assert_eq!(tc.as_deref(), Some("nightly-2026-04-15"));
    let uses = msrv.steps.iter().find_map(|s| s.uses.clone().filter(|u| u.starts_with(ACTION)));
    assert_eq!(uses, Some(format!("{ACTION}{PINNED_SHA}")));
}

// ── refuse: the exact PR #1 head state and its predecessor ───────────────────

#[test]
fn pr1_head_bump_to_nonexistent_1_100_is_refused() {
    let err = refused(&mutate_msrv(&real_ci(), "      - uses: dtolnay/rust-toolchain@1.100\n"));
    assert!(err.contains(&Violation::NumericToolchainRef {
        job: "msrv".into(),
        reference: "1.100".into()
    }));
    assert!(err.contains(&Violation::MsrvToolchainDrift {
        job: "1.100".into(),
        pinned: "nightly-2026-04-15".into()
    }));
}

#[test]
fn base_main_stable_1_82_pin_is_refused() {
    let err = refused(&mutate_msrv(&real_ci(), "      - uses: dtolnay/rust-toolchain@1.82\n"));
    assert!(err.contains(&Violation::NumericToolchainRef {
        job: "msrv".into(),
        reference: "1.82".into()
    }));
}

#[test]
fn stale_nightly_in_msrv_job_is_refused() {
    let bad =
        mutate_msrv(&real_ci(), &msrv_block().replace("nightly-2026-04-15", "nightly-2026-04-14"));
    assert_eq!(
        check(&bad, &real_toolchain()),
        Err(vec![Violation::MsrvToolchainDrift {
            job: "nightly-2026-04-14".into(),
            pinned: "nightly-2026-04-15".into()
        }])
    );
}

#[test]
fn toolchain_file_moved_without_ci_is_refused() {
    let moved = real_toolchain().replace("nightly-2026-04-15", "nightly-2026-05-04");
    let err = check(&real_ci(), &moved).expect_err("toolchain-file bump alone is drift");
    assert!(matches!(err.as_slice(), [Violation::MsrvToolchainDrift { .. }]));
}

#[test]
fn master_ref_without_toolchain_input_is_refused() {
    let err = refused(&mutate_msrv(&real_ci(), "      - uses: dtolnay/rust-toolchain@master\n"));
    assert!(err.contains(&Violation::RefWithoutToolchain {
        job: "msrv".into(),
        reference: "master".into()
    }));
}

#[test]
fn floating_master_ref_in_msrv_is_refused() {
    let bad = mutate_msrv(&real_ci(), &msrv_block().replace(PINNED_SHA, "master"));
    assert_eq!(
        check(&bad, &real_toolchain()),
        Err(vec![Violation::MsrvActionNotShaPinned { reference: "master".into() }])
    );
}

#[test]
fn numeric_ref_in_any_job_is_refused() {
    let bad =
        real_ci().replacen("dtolnay/rust-toolchain@stable", "dtolnay/rust-toolchain@1.97.1", 1);
    let err = refused(&bad);
    assert!(err.iter().any(
        |e| matches!(e, Violation::NumericToolchainRef { reference, .. } if reference == "1.97.1")
    ));
}

// ── refuse: YAML forms the first parser missed (court findings on e6eaba1) ───

#[test]
fn extra_spaces_after_dash_do_not_hide_a_numeric_ref() {
    let err =
        refused(&append_to_msrv(&real_ci(), "      -   uses: dtolnay/rust-toolchain@1.100\n"));
    assert!(err.contains(&Violation::NumericToolchainRef {
        job: "msrv".into(),
        reference: "1.100".into()
    }));
    assert!(err.contains(&Violation::MsrvToolchainDrift {
        job: "1.100".into(),
        pinned: "nightly-2026-04-15".into()
    }));
}

#[test]
fn flow_style_step_does_not_hide_a_numeric_ref() {
    let err =
        refused(&append_to_msrv(&real_ci(), "      - {uses: dtolnay/rust-toolchain@1.100}\n"));
    assert!(err.contains(&Violation::NumericToolchainRef {
        job: "msrv".into(),
        reference: "1.100".into()
    }));
    assert!(err.contains(&Violation::MsrvToolchainDrift {
        job: "1.100".into(),
        pinned: "nightly-2026-04-15".into()
    }));
}

#[test]
fn flow_style_with_input_is_checked() {
    let extra = format!(
        "      - {{uses: dtolnay/rust-toolchain@{PINNED_SHA}, with: {{toolchain: '1.100'}}}}\n"
    );
    let err = refused(&append_to_msrv(&real_ci(), &extra));
    assert!(err
        .contains(&Violation::NumericToolchainInput { job: "msrv".into(), value: "1.100".into() }));
    assert!(err.contains(&Violation::MsrvToolchainDrift {
        job: "1.100".into(),
        pinned: "nightly-2026-04-15".into()
    }));
}

#[test]
fn second_install_step_with_numeric_input_is_refused() {
    let extra = "      - uses: dtolnay/rust-toolchain@master\n        with:\n          toolchain: '1.100'\n";
    let err = refused(&append_to_msrv(&real_ci(), extra));
    assert!(err
        .contains(&Violation::NumericToolchainInput { job: "msrv".into(), value: "1.100".into() }));
    assert!(err.contains(&Violation::MsrvToolchainDrift {
        job: "1.100".into(),
        pinned: "nightly-2026-04-15".into()
    }));
    assert!(err.contains(&Violation::MsrvActionNotShaPinned { reference: "master".into() }));
}

#[test]
fn rustup_toolchain_env_at_job_level_is_refused() {
    let bad = real_ci().replacen(
        "  msrv:\n    name: msrv\n",
        "  msrv:\n    name: msrv\n    env:\n      RUSTUP_TOOLCHAIN: \"1.100\"\n",
        1,
    );
    assert_eq!(
        check(&bad, &real_toolchain()),
        Err(vec![Violation::ToolchainEnvOverride { value: "1.100".into() }])
    );
}

#[test]
fn rustup_toolchain_env_in_every_other_form_is_refused() {
    let forms = [
        // workflow-level env
        real_ci().replacen("env:\n", "env:\n  RUSTUP_TOOLCHAIN: stable\n", 1),
        // step-level flow env
        append_to_msrv(
            &real_ci(),
            "      - run: cargo check\n        env: {RUSTUP_TOOLCHAIN: '1.100'}\n",
        ),
        // exported through $GITHUB_ENV by a run step
        append_to_msrv(&real_ci(), "      - run: echo RUSTUP_TOOLCHAIN=1.100 >> $GITHUB_ENV\n"),
    ];
    for bad in forms {
        let err = refused(&bad);
        assert!(err.iter().any(|e| matches!(e, Violation::ToolchainEnvOverride { .. })), "{err:?}");
    }
    // Setting it to the pinned channel itself is not an override.
    let same = append_to_msrv(
        &real_ci(),
        "      - run: cargo check\n        env:\n          RUSTUP_TOOLCHAIN: nightly-2026-04-15\n",
    );
    assert_eq!(check(&same, &real_toolchain()), Ok(()));
}

#[test]
fn toolchain_selecting_commands_in_msrv_are_refused() {
    for cmd in ["cargo +1.100 check", "rustup default 1.100", "rustup override set stable"] {
        let err = refused(&append_to_msrv(&real_ci(), &format!("      - run: {cmd}\n")));
        assert!(
            err.contains(&Violation::ToolchainCommandOverride { line: format!("- run: {cmd}") }),
            "{cmd}: {err:?}"
        );
    }
}

#[test]
fn unmodelled_yaml_form_is_refused_by_the_lexical_backstop() {
    // A folded block scalar: the parser records `uses: >-`, the lexical scan
    // still sees the reference, so the counts disagree.
    let extra = "      - name: folded\n        uses: >-\n          dtolnay/rust-toolchain@1.100\n";
    let err = refused(&append_to_msrv(&real_ci(), extra));
    assert!(err.contains(&Violation::UnparsedToolchainRef {
        job: "msrv".into(),
        lexical: 2,
        parsed: 1
    }));
    assert!(err.contains(&Violation::NumericToolchainRef {
        job: "msrv".into(),
        reference: "1.100".into()
    }));
}

#[test]
fn commented_out_refs_are_not_counted() {
    let extra = "      # - uses: dtolnay/rust-toolchain@1.100\n";
    assert_eq!(check(&append_to_msrv(&real_ci(), extra), &real_toolchain()), Ok(()));
}

// ── refuse: malformed / structural adversaries ───────────────────────────────

#[test]
fn missing_jobs_mapping_is_refused() {
    assert_eq!(check("name: CI\non: push\n", &real_toolchain()), Err(vec![Violation::JobsMissing]));
}

#[test]
fn indented_jobs_key_is_not_top_level() {
    let bad = real_ci().replacen("\njobs:\n", "\n  jobs:\n", 1);
    assert!(refused(&bad).contains(&Violation::JobsMissing));
}

#[test]
fn missing_msrv_job_is_refused() {
    let bad = real_ci().replacen("  msrv:\n    name: msrv\n", "  minver:\n    name: minver\n", 1);
    assert!(refused(&bad).contains(&Violation::MsrvJobCount(0)));
}

#[test]
fn duplicate_msrv_job_is_refused() {
    let dup = format!(
        "  msrv:\n    name: msrv-dup\n    runs-on: ubuntu-latest\n    steps:\n{}",
        msrv_block()
    );
    let bad = format!("{}\n{dup}", real_ci().trim_end_matches('\n'));
    assert!(refused(&bad).contains(&Violation::MsrvJobCount(2)));
}

#[test]
fn msrv_job_without_toolchain_step_is_refused() {
    let bad = mutate_msrv(&real_ci(), "");
    assert!(refused(&bad).contains(&Violation::MsrvToolchainMissing));
}

#[test]
fn floating_or_malformed_channels_are_refused() {
    for ch in
        ["nightly", "stable", "beta", "nightly-2026-4-15", "nightly-20260415", "1.", "1.x", ""]
    {
        let toml = format!("[toolchain]\nchannel = \"{ch}\"\n");
        let err = check(&real_ci(), &toml).expect_err(ch);
        assert!(err.contains(&Violation::ChannelNotPinned(ch.to_string())), "{ch}: {err:?}");
    }
    let err = check(&real_ci(), "[toolchain]\n# channel = \"nightly-2026-04-15\"\n")
        .expect_err("commented");
    assert!(err.contains(&Violation::ChannelMissing));
}

#[test]
fn quoting_and_trailing_comments_do_not_hide_drift() {
    let bad = mutate_msrv(&real_ci(), "      - uses: 'dtolnay/rust-toolchain@1.100' # bumped\n");
    let err = refused(&bad);
    assert!(err.iter().any(
        |e| matches!(e, Violation::NumericToolchainRef { reference, .. } if reference == "1.100")
    ));
}

#[test]
fn dependabot_ignores_rust_toolchain_refs() {
    let d = repo_file(".github/dependabot.yml");
    let gha =
        d.split("package-ecosystem: \"github-actions\"").nth(1).expect("github-actions ecosystem");
    let gha = gha.split("package-ecosystem:").next().unwrap_or(gha);
    assert!(
        gha.contains("dependency-name: \"dtolnay/rust-toolchain\""),
        "github-actions updates must ignore dtolnay/rust-toolchain"
    );
}

// ── witness: the real toolchain resolved by rustup is the pinned one ─────────

#[test]
fn rustup_resolves_the_pinned_channel_in_this_checkout() {
    let pinned = pinned_channel(&real_toolchain()).expect("pinned");
    let out = Command::new("rustup")
        .args(["show", "active-toolchain"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("RUSTUP_TOOLCHAIN")
        .output();
    let Ok(out) = out else {
        eprintln!("SKIP(named): rustup not on PATH; witness not taken");
        return;
    };
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.starts_with(&pinned), "active toolchain {s:?} does not start with pinned {pinned}");
}

// ── benchmark: bounded admission cost ────────────────────────────────────────

#[test]
fn guard_cost_is_bounded() {
    let ci = real_ci();
    let tc = real_toolchain();
    let (batches, per_batch) = (20u32, 100u32);
    for _ in 0..50 {
        assert!(check(&ci, &tc).is_ok());
    }
    // Minimum over batches: the least-contended estimate of the guard's own
    // cost, so a saturated host (other builds) does not masquerade as a
    // regression, while a quadratic parse still inflates every batch.
    let mut best = std::time::Duration::MAX;
    let mut total = std::time::Duration::ZERO;
    for _ in 0..batches {
        let t0 = Instant::now();
        for _ in 0..per_batch {
            assert!(check(std::hint::black_box(&ci), std::hint::black_box(&tc)).is_ok());
        }
        let el = t0.elapsed();
        total += el;
        best = best.min(el / per_batch);
    }
    let mean = total / (batches * per_batch);
    eprintln!(
        "BENCH ci_toolchain_guard check: min {} ns/iter, mean {} ns/iter over {} iters",
        best.as_nanos(),
        mean.as_nanos(),
        batches * per_batch
    );
    // Regression bound on the minimum batch: 2 ms/iter leaves headroom for
    // slow CI runners while catching accidental quadratic parses.
    assert!(best.as_micros() < 2_000, "guard check took {best:?}/iter");
}
