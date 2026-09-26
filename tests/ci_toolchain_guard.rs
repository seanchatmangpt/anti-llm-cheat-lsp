//! CI toolchain guard (hardening for PR #1, dependabot bump of
//! `dtolnay/rust-toolchain` from `@1.82` to `@1.100`).
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

#![allow(clippy::expect_used, clippy::unwrap_used)] // test code: a panic is the failure signal

use std::{path::Path, process::Command, time::Instant};

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
    /// The `msrv` job's toolchain differs from `rust-toolchain.toml`.
    MsrvToolchainDrift { job: String, pinned: String },
    /// A step uses `dtolnay/rust-toolchain@<numeric>`; the ref encodes a Rust
    /// version (bump bots move it, and a numeric stable cannot build the crate).
    NumericToolchainRef { job: String, reference: String },
    /// `dtolnay/rust-toolchain@master` without a `toolchain:` input.
    MasterWithoutToolchain { job: String },
}

#[derive(Debug, Default)]
struct Step {
    uses: Option<String>,
    toolchain: Option<String>,
}

#[derive(Debug)]
struct Job {
    key: String,
    steps: Vec<Step>,
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn strip_value(v: &str) -> String {
    let v = v.split(" #").next().unwrap_or(v).trim();
    v.trim_matches('"').trim_matches('\'').to_string()
}

/// Minimal indentation-driven extractor for the GitHub Actions subset used in
/// this repository (block mappings, `- ` step sequences, `with:` blocks).
fn parse_jobs(yaml: &str) -> Result<Vec<Job>, Violation> {
    let lines: Vec<&str> = yaml.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_end() == "jobs:" && indent_of(l) == 0)
        .ok_or(Violation::JobsMissing)?;
    let mut jobs: Vec<Job> = Vec::new();
    let mut job_indent: Option<usize> = None;
    let mut step_indent: Option<usize> = None;
    let mut in_with = false;
    let mut with_indent = 0usize;
    for raw in &lines[start + 1..] {
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let ind = indent_of(raw);
        if ind == 0 {
            break; // next top-level key ends `jobs:`
        }
        let ji = *job_indent.get_or_insert(ind);
        if ind == ji {
            let key = t.trim_end_matches(':').to_string();
            jobs.push(Job { key, steps: Vec::new() });
            step_indent = None;
            in_with = false;
            continue;
        }
        let Some(job) = jobs.last_mut() else { continue };
        if let Some(rest) = t.strip_prefix("- ") {
            if step_indent.is_none() || Some(ind) == step_indent {
                step_indent = Some(ind);
                job.steps.push(Step::default());
                in_with = false;
                let step = job.steps.last_mut().expect("just pushed");
                if let Some(u) = rest.strip_prefix("uses:") {
                    step.uses = Some(strip_value(u));
                }
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
            if t == "with:" {
                in_with = true;
                with_indent = ind;
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
    let numeric = {
        let p: Vec<&str> = channel.split('.').collect();
        (2..=3).contains(&p.len())
            && p.iter().all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
    };
    if dated_nightly || numeric {
        Ok(channel)
    } else {
        Err(Violation::ChannelNotPinned(channel))
    }
}

fn is_numeric_ref(r: &str) -> bool {
    !r.is_empty() && r.split('.').all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
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
        for step in &job.steps {
            let Some(uses) = &step.uses else { continue };
            let Some(reference) = uses.strip_prefix("dtolnay/rust-toolchain@") else { continue };
            if is_numeric_ref(reference) {
                v.push(Violation::NumericToolchainRef {
                    job: job.key.clone(),
                    reference: reference.to_string(),
                });
            }
            if reference == "master" && step.toolchain.is_none() {
                v.push(Violation::MasterWithoutToolchain { job: job.key.clone() });
            }
        }
    }
    let msrv: Vec<&Job> = jobs.iter().filter(|j| j.key == "msrv").collect();
    if msrv.len() == 1 {
        let effective = msrv[0].steps.iter().find_map(|s| {
            let r = s.uses.as_deref()?.strip_prefix("dtolnay/rust-toolchain@")?;
            Some(s.toolchain.clone().unwrap_or_else(|| r.to_string()))
        });
        match (effective, &pinned) {
            (None, _) => v.push(Violation::MsrvToolchainMissing),
            (Some(job), Some(p)) if &job != p => {
                v.push(Violation::MsrvToolchainDrift { job, pinned: p.clone() });
            }
            _ => {}
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

/// Replace the msrv job's toolchain install lines with `replacement`.
fn mutate_msrv(ci: &str, replacement: &str) -> String {
    let marker = "      - uses: dtolnay/rust-toolchain@master\n        with:\n          toolchain: nightly-2026-04-15\n";
    assert!(ci.contains(marker), "fixture drift: msrv install block not found in ci.yml");
    ci.replacen(marker, replacement, 1)
}

// ── admit: the committed subject ─────────────────────────────────────────────

#[test]
fn committed_ci_and_toolchain_are_admitted() {
    assert_eq!(check(&real_ci(), &real_toolchain()), Ok(()));
}

#[test]
fn parser_sees_every_committed_job() {
    let jobs = parse_jobs(&real_ci()).expect("jobs");
    let keys: Vec<&str> = jobs.iter().map(|j| j.key.as_str()).collect();
    for k in ["fmt", "clippy", "test", "doc", "msrv", "deny", "typos", "coverage", "security-audit"]
    {
        assert!(keys.contains(&k), "job {k} missing from parse: {keys:?}");
    }
    let msrv = jobs.iter().find(|j| j.key == "msrv").expect("msrv");
    let tc = msrv.steps.iter().find_map(|s| s.toolchain.clone());
    assert_eq!(tc.as_deref(), Some("nightly-2026-04-15"));
}

// ── refuse: the exact PR #1 head state and its predecessor ───────────────────

#[test]
fn pr1_head_bump_to_nonexistent_1_100_is_refused() {
    let bad = mutate_msrv(&real_ci(), "      - uses: dtolnay/rust-toolchain@1.100\n");
    let err = check(&bad, &real_toolchain()).expect_err("1.100 must be refused");
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
    let bad = mutate_msrv(&real_ci(), "      - uses: dtolnay/rust-toolchain@1.82\n");
    let err = check(&bad, &real_toolchain()).expect_err("1.82 cannot build a #![feature] crate");
    assert!(err.contains(&Violation::NumericToolchainRef {
        job: "msrv".into(),
        reference: "1.82".into()
    }));
}

#[test]
fn stale_nightly_in_msrv_job_is_refused() {
    let bad = mutate_msrv(
        &real_ci(),
        "      - uses: dtolnay/rust-toolchain@master\n        with:\n          toolchain: nightly-2026-04-14\n",
    );
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
    let bad = mutate_msrv(&real_ci(), "      - uses: dtolnay/rust-toolchain@master\n");
    let err = check(&bad, &real_toolchain()).expect_err("master needs toolchain input");
    assert!(err.contains(&Violation::MasterWithoutToolchain { job: "msrv".into() }));
}

#[test]
fn numeric_ref_in_any_job_is_refused() {
    let bad =
        real_ci().replacen("dtolnay/rust-toolchain@stable", "dtolnay/rust-toolchain@1.97.1", 1);
    let err = check(&bad, &real_toolchain()).expect_err("numeric ref outside msrv");
    assert!(err.iter().any(
        |e| matches!(e, Violation::NumericToolchainRef { reference, .. } if reference == "1.97.1")
    ));
}

// ── refuse: malformed / structural adversaries ───────────────────────────────

#[test]
fn missing_jobs_mapping_is_refused() {
    assert_eq!(check("name: CI\non: push\n", &real_toolchain()), Err(vec![Violation::JobsMissing]));
}

#[test]
fn indented_jobs_key_is_not_top_level() {
    let bad = real_ci().replacen("\njobs:\n", "\n  jobs:\n", 1);
    assert!(check(&bad, &real_toolchain())
        .expect_err("nested jobs")
        .contains(&Violation::JobsMissing));
}

#[test]
fn missing_msrv_job_is_refused() {
    let bad = real_ci().replacen("  msrv:\n    name: msrv\n", "  minver:\n    name: minver\n", 1);
    assert!(check(&bad, &real_toolchain())
        .expect_err("no msrv")
        .contains(&Violation::MsrvJobCount(0)));
}

#[test]
fn duplicate_msrv_job_is_refused() {
    let dup = "  msrv:\n    name: msrv-dup\n    runs-on: ubuntu-latest\n    steps:\n      - uses: dtolnay/rust-toolchain@master\n        with:\n          toolchain: nightly-2026-04-15\n";
    let bad = format!("{}{}", real_ci().trim_end_matches('\n').to_string() + "\n", dup);
    assert!(check(&bad, &real_toolchain())
        .expect_err("dup msrv")
        .contains(&Violation::MsrvJobCount(2)));
}

#[test]
fn msrv_job_without_toolchain_step_is_refused() {
    let bad = mutate_msrv(&real_ci(), "");
    assert!(check(&bad, &real_toolchain())
        .expect_err("no install")
        .contains(&Violation::MsrvToolchainMissing));
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
    let err = check(&bad, &real_toolchain()).expect_err("quoted numeric ref");
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
    let iters = 2_000u32;
    for _ in 0..50 {
        assert!(check(&ci, &tc).is_ok());
    }
    let t0 = Instant::now();
    for _ in 0..iters {
        assert!(check(std::hint::black_box(&ci), std::hint::black_box(&tc)).is_ok());
    }
    let per = t0.elapsed() / iters;
    eprintln!("BENCH ci_toolchain_guard check: {} ns/iter over {iters} iters", per.as_nanos());
    // Regression bound: measured ~tens of microseconds in debug; 2 ms leaves
    // headroom for slow CI runners while catching accidental quadratic parses.
    assert!(per.as_micros() < 2_000, "guard check took {per:?}/iter");
}
