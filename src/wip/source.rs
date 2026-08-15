use std::{
    fs,
    path::{Path, PathBuf},
};

use ignore::WalkBuilder;
use regex::Regex;

use super::{
    model::{SourceFinding, Standing, WipKind},
    tree_sitter_scan::scan_tree_sitter_file,
};

pub fn scan_source_wip(root: impl AsRef<Path>) -> Vec<SourceFinding> {
    let root = root.as_ref();
    let mut findings = Vec::new();
    let marker_rules: [(&str, WipKind, Standing, &str); 7] = [
        (
            "TODO",
            WipKind::SourceMarker,
            Standing::PartialAlive,
            "TODO marker",
        ),
        (
            "FIXME",
            WipKind::SourceMarker,
            Standing::PartialAlive,
            "FIXME marker",
        ),
        (
            "todo!(",
            WipKind::Code,
            Standing::PartialAlive,
            "Rust todo! macro",
        ),
        (
            "unimplemented!(",
            WipKind::Code,
            Standing::PartialAlive,
            "Rust unimplemented! macro",
        ),
        (
            "file:///",
            WipKind::Replay,
            Standing::PartialAlive,
            "machine-local replay pointer",
        ),
        (
            "stub implementation",
            WipKind::Code,
            Standing::PartialAlive,
            "stub implementation marker",
        ),
        (
            "not implemented",
            WipKind::Code,
            Standing::PartialAlive,
            "not-implemented marker",
        ),
    ];

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.filter_map(Result::ok) {
        let path = entry.path();
        if should_skip(path) || !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if is_scanner_self_surface(&relative) {
            continue;
        }
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        // Supported programming languages are AST-first. This is an authority
        // fence against treating marker-like strings or generated syntax as WIP.
        // The line scanner below is reserved for prose/config/unsupported files.
        if let Some(ast_findings) = scan_tree_sitter_file(path, &relative, &content) {
            findings.extend(ast_findings);
            continue;
        }

        for (index, line) in content.lines().enumerate() {
            if line.contains("WipKind::") && line.contains("Standing::") {
                continue;
            }
            for &(marker, kind, standing, label) in &marker_rules {
                if line.contains(marker) {
                    findings.push(SourceFinding {
                        id: finding_id(&relative, index + 1, marker),
                        path: relative.clone(),
                        line: index + 1,
                        marker: marker.to_string(),
                        kind,
                        standing,
                        message: format!("{label}: {}", compact_line(line)),
                    });
                }
            }
            if let Some((marker, kind, standing, label)) = classify_declared_status(line) {
                findings.push(SourceFinding {
                    id: finding_id(&relative, index + 1, marker),
                    path: relative.clone(),
                    line: index + 1,
                    marker: marker.to_string(),
                    kind,
                    standing,
                    message: format!("{label}: {}", compact_line(line)),
                });
            }
        }
    }

    findings.extend(scan_missing_path_dependencies(root));
    findings.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.marker.cmp(&b.marker))
    });
    findings.dedup_by(|a, b| a.id == b.id);
    findings
}

fn classify_declared_status(line: &str) -> Option<(&'static str, WipKind, Standing, &'static str)> {
    let normalized = line.trim().to_ascii_uppercase();
    let declares_status = normalized.starts_with("STATUS:")
        || normalized.starts_with("STANDING:")
        || normalized.starts_with("STATUS =")
        || normalized.starts_with("STANDING =")
        || normalized.starts_with("| STATUS |")
        || normalized.starts_with("| STANDING |");
    if !declares_status {
        return None;
    }

    if normalized.contains("BUILD_BROKEN") {
        Some((
            "BUILD_BROKEN",
            WipKind::Ci,
            Standing::BuildBroken,
            "declared broken build",
        ))
    } else if normalized.contains("BLOCKED") {
        Some((
            "BLOCKED",
            WipKind::CrossRepoBlocker,
            Standing::Blocked,
            "declared blocker",
        ))
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

fn scan_missing_path_dependencies(root: &Path) -> Vec<SourceFinding> {
    let manifest = root.join("Cargo.toml");
    let content = match fs::read_to_string(&manifest) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };
    let path_re = match Regex::new(r#"path\s*=\s*"([^"]+)""#) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    let mut findings = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        for captures in path_re.captures_iter(line) {
            let Some(path_match) = captures.get(1) else {
                continue;
            };
            let dep_path = path_match.as_str();
            let resolved = normalize_join(root, dep_path);
            if resolved.exists() {
                continue;
            }
            findings.push(SourceFinding {
                id: finding_id("Cargo.toml", line_index + 1, dep_path),
                path: "Cargo.toml".to_string(),
                line: line_index + 1,
                marker: dep_path.to_string(),
                kind: WipKind::Dependency,
                standing: Standing::Blocked,
                message: format!("missing path dependency: {dep_path}"),
            });
        }
    }
    findings
}

fn normalize_join(root: &Path, relative: &str) -> PathBuf {
    root.join(relative)
}

pub(super) fn finding_id(path: &str, line: usize, marker: &str) -> String {
    let digest = blake3::hash(format!("{path}:{line}:{marker}").as_bytes());
    let hex = digest.to_hex().to_string();
    format!("source:{}", &hex[..16])
}

fn compact_line(line: &str) -> String {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    let prefix: String = chars.by_ref().take(160).collect();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn is_scanner_self_surface(relative: &str) -> bool {
    matches!(
        relative,
        "src/wip/source.rs"
            | "src/wip/tree_sitter_scan.rs"
            | "tests/wip_closure.rs"
            | "docs/WIP_CLOSURE_ENGINE.md"
    )
}

fn should_skip(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_string_lossy().as_ref(),
            ".git"
                | "target"
                | "node_modules"
                | "vendor"
                | "generated"
                | "receipts"
                | "transcripts"
                | "ocel"
        )
    })
}
