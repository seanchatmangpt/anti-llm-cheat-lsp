//! Cross-product #3: the `ggen://` virtual-document scheme.
//!
//! Serves, on demand via `workspace/textDocumentContent`, a ggen-rendered
//! artifact for an ontology URI that is NEVER written to disk. The served
//! virtual document carries two load-bearing facts:
//!
//!   (a) its MONIKER identity using the Phase-A convention — the same
//!       `scheme:identifier` content address an on-disk equivalent of the
//!       same symbol would carry (see [`moniker_object_id`]); and
//!   (b) a RECEIPT whose digest is computed over the actual rendered bytes.
//!
//! `ggen://wip-closure` is a reserved dynamic observation surface. It renders
//! the read-only Little's Law WIP report instead of an ontology artifact and
//! never grants execution authority to its closure intents.
//!
//! Live invocation of the real `ggen` binary at runtime is OUT OF SCOPE here:
//! the rendering below is a deterministic representative artifact produced from
//! an in-repo embedded ontology + template. The bounded status for live-ggen
//! invocation is therefore `OPEN` (see [`LIVE_GGEN_STATUS`]). The receipt is
//! NOT faked — its digest is taken over the bytes this module actually emits.

use std::path::Path;

use chrono::Utc;

use crate::wip::{self, GitHubSnapshot, WIP_SNAPSHOT_SCHEMA};

/// Virtual-document scheme handled by this surface.
pub const SCHEME: &str = "ggen";

/// Reserved URI for the live, read-only WIP closure report.
pub const WIP_CLOSURE_URI: &str = "ggen://wip-closure";

/// Conventional workspace-relative admitted GitHub snapshot path.
pub const WIP_SNAPSHOT_PATH: &str = ".chatmangpt/wip-snapshot.json";

/// Bounded status of LIVE `ggen` binary invocation at runtime. This module
/// renders a deterministic representative artifact from an embedded ontology;
/// it does not shell out to a real `ggen`. Hence `OPEN`, never `ADMITTED`.
pub const LIVE_GGEN_STATUS: &str = "OPEN";

/// Embedded in-repo ontology (representative). Content-addressed; never read
/// from disk so the virtual document has no on-disk dependency.
pub const EMBEDDED_ONTOLOGY: &str = "\
@prefix ex: <https://example.org/onto#> .
ex:Widget a ex:Class ; ex:label \"Widget\" .
ex:Gadget a ex:Class ; ex:label \"Gadget\" .
";

/// Embedded representative template. A real ggen run would consume the paired
/// SPARQL projection; here we emit a deterministic projection of the ontology.
pub const EMBEDDED_TEMPLATE: &str = "// rendered-from: {ontology_uri}\n{rows}\n";

/// Phase-A content address for an ontology URI.
///
/// This is the SAME formula an on-disk equivalent of the same symbol would
/// use: the identity is a content address derived from the NORMALIZED ontology
/// URI, never from any numeric vertex id, file path, or buffer location. A
/// virtual document and an on-disk document describing the same ontology URI
/// therefore reduce to one identity. The witness test pins exactly this.
pub fn moniker_object_id(ontology_uri: &str) -> String {
    let normalized = normalize_uri(ontology_uri);
    let digest = blake3::hash(normalized.as_bytes()).to_hex();
    // Phase-A convention: `scheme:identifier` content address.
    format!("{SCHEME}:{digest}")
}

/// Normalize an ontology URI so that the content address is stable across
/// trivial representational differences (trailing slash, surrounding space).
/// Path-independent: a `file://` on-disk URI and a `ggen://` virtual URI for
/// the SAME ontology symbol normalize to the same identifier.
fn normalize_uri(uri: &str) -> String {
    let trimmed = uri.trim();
    let trimmed = trimmed.strip_suffix('/').unwrap_or(trimmed);
    // Strip the transport scheme so on-disk vs virtual transports collapse to
    // one symbol identity; the ontology authority+path is the symbol.
    let symbol = trimmed.split_once("://").map(|(_scheme, rest)| rest).unwrap_or(trimmed);
    symbol.to_string()
}

/// A rendered virtual document: the served text plus its moniker and receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedVirtualDoc {
    /// Phase-A moniker content address (`ggen:<digest>`).
    pub moniker: String,
    /// The rendered artifact bytes, as served to the client. Never on disk.
    pub text: String,
    /// Receipt over the rendered bytes (digest algorithm, digest, status).
    pub receipt: Receipt,
}

/// Receipt for a rendered virtual document. The digest is computed over the
/// actual served bytes; `live_ggen` records that runtime ggen invocation is
/// `OPEN`, so no live-rendering claim is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub moniker: String,
    pub digest_algorithm: String,
    pub digest: String,
    pub boundary: String,
    pub live_ggen: String,
    pub status: String,
}

/// Render the deterministic representative artifact for an ontology URI.
///
/// The same `ontology_uri` always yields the same moniker, text, and digest.
pub fn render(ontology_uri: &str) -> RenderedVirtualDoc {
    let moniker = moniker_object_id(ontology_uri);
    let rows = project_ontology(EMBEDDED_ONTOLOGY);
    let text = EMBEDDED_TEMPLATE.replace("{ontology_uri}", ontology_uri).replace("{rows}", &rows);

    let digest = blake3::hash(text.as_bytes()).to_hex().to_string();
    let receipt = Receipt {
        moniker: moniker.clone(),
        digest_algorithm: "blake3".to_string(),
        digest,
        boundary: "virtual-document/ggen".to_string(),
        live_ggen: LIVE_GGEN_STATUS.to_string(),
        // Artifact bytes admitted with receipt; live-ggen path remains OPEN.
        status: "CANDIDATE".to_string(),
    };

    RenderedVirtualDoc { moniker, text, receipt }
}

/// Deterministic projection of the embedded ontology into template rows.
/// Sorted so the rendered bytes (and thus the receipt digest) are stable.
fn project_ontology(ontology: &str) -> String {
    let mut classes: Vec<&str> = ontology
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("ex:")
                .and_then(|rest| rest.split_whitespace().next())
                .filter(|_| line.contains("a ex:Class"))
        })
        .collect();
    classes.sort_unstable();
    classes.dedup();
    classes.into_iter().map(|c| format!("class ex:{c}")).collect::<Vec<_>>().join("\n")
}

/// Render the virtual document as the markdown surface served over LSP. Carries
/// the moniker identity and the receipt inline so the client sees both facts.
pub fn generate_ggen_markdown(ontology_uri: &str) -> String {
    if ontology_uri == WIP_CLOSURE_URI {
        return generate_wip_closure_markdown();
    }

    let doc = render(ontology_uri);
    let r = &doc.receipt;
    let mut out = String::new();
    out.push_str("# ggen:// Virtual Document\n\n");
    out.push_str(&format!("Ontology URI: `{ontology_uri}`\n\n"));
    out.push_str(&format!("Moniker (Phase-A): `{}`\n\n", doc.moniker));
    out.push_str("## Receipt\n\n");
    out.push_str("| Field | Value |\n| --- | --- |\n");
    out.push_str(&format!("| moniker | `{}` |\n", r.moniker));
    out.push_str(&format!("| digest_algorithm | {} |\n", r.digest_algorithm));
    out.push_str(&format!("| digest | `{}` |\n", r.digest));
    out.push_str(&format!("| boundary | {} |\n", r.boundary));
    out.push_str(&format!("| live_ggen | {} |\n", r.live_ggen));
    out.push_str(&format!("| status | {} |\n\n", r.status));
    out.push_str("## Rendered Artifact (never written to disk)\n\n");
    out.push_str("```\n");
    out.push_str(&doc.text);
    out.push_str("```\n");
    out
}

/// Render the WIP report through the already-wired `ggen://` LSP transport.
///
/// The current process working directory is the bounded source root because the
/// existing text-document-content router does not pass `workspace_root` into
/// this module. If `.chatmangpt/wip-snapshot.json` exists it must pass the WIP
/// schema gate; otherwise the view is explicitly source-only. No closure intent
/// is executed here.
fn generate_wip_closure_markdown() -> String {
    let snapshot_path = Path::new(WIP_SNAPSHOT_PATH);
    let (snapshot, observation_boundary) = if snapshot_path.is_file() {
        match wip::load_snapshot(snapshot_path) {
            Ok(snapshot) => {
                (snapshot, format!("source + admitted GitHub snapshot `{WIP_SNAPSHOT_PATH}`"))
            }
            Err(error) => {
                return format!(
                    "# Little's Law WIP Closure Report\n\nStatus: **REFUSED**\n\nSnapshot `{WIP_SNAPSHOT_PATH}` was rejected: {error}\n\nNo closure intent was constructed from the rejected snapshot.\n"
                );
            }
        }
    } else {
        (
            GitHubSnapshot {
                schema_version: WIP_SNAPSHOT_SCHEMA.to_string(),
                observed_at: Utc::now(),
                window_days: 30,
                workspace_repository: None,
                repositories: Vec::new(),
            },
            "source-only; no admitted GitHub snapshot present".to_string(),
        )
    };

    let report = wip::analyze_path(&snapshot, ".");
    let mut out = report.to_markdown();
    out.push_str("\n## LSP observation boundary\n\n");
    out.push_str(&format!("- URI: `{WIP_CLOSURE_URI}`\n"));
    out.push_str(&format!("- Observation: {observation_boundary}\n"));
    out.push_str("- Source root: current process working directory (`.`)\n");
    out.push_str("- Authority: `INTENT_ONLY`; this virtual document never actuates a closure\n");
    out
}

#[cfg(test)]
mod witness {
    use super::*;

    /// WITNESS — moniker PARITY across transports.
    ///
    /// The virtual-doc path and an on-disk equivalent of the SAME ontology
    /// symbol must derive ONE identity. This reduces to the Phase-A
    /// content-address property: identity is a function of the normalized
    /// ontology symbol, NOT of the transport scheme or file path. The test
    /// FAILS if the virtual-doc path derives a different moniker than the
    /// on-disk path for the same symbol.
    #[test]
    fn moniker_parity_virtual_equals_on_disk() {
        // Same ontology symbol, two transports: a virtual `ggen://` URI and an
        // on-disk `file://` URI. Phase-A identity must collapse them.
        let virtual_uri = "ggen://example.org/onto/widgets";
        let on_disk_uri = "file://example.org/onto/widgets";

        let virtual_id = moniker_object_id(virtual_uri);
        let on_disk_id = moniker_object_id(on_disk_uri);

        assert_eq!(
            virtual_id, on_disk_id,
            "moniker PARITY violated: virtual-doc identity {virtual_id} \
             diverged from on-disk identity {on_disk_id} for the same symbol"
        );

        // The served virtual document must carry exactly this identity.
        let doc = render(virtual_uri);
        assert_eq!(
            doc.moniker, virtual_id,
            "served virtual doc moniker {} did not match Phase-A identity {virtual_id}",
            doc.moniker
        );
    }

    /// The receipt digest is taken over the ACTUAL served bytes — not faked.
    #[test]
    fn receipt_digest_covers_served_bytes() {
        let doc = render("ggen://example.org/onto/widgets");
        let recomputed = blake3::hash(doc.text.as_bytes()).to_hex().to_string();
        assert_eq!(
            doc.receipt.digest, recomputed,
            "receipt digest must be computed over the served bytes"
        );
        assert_eq!(doc.receipt.digest_algorithm, "blake3");
    }

    /// Live-ggen runtime invocation is bounded OPEN, never an admission.
    #[test]
    fn live_ggen_is_open_not_admitted() {
        let doc = render("ggen://example.org/onto/widgets");
        assert_eq!(doc.receipt.live_ggen, "OPEN");
        assert_ne!(doc.receipt.status, "ADMITTED");
    }

    /// Determinism: same symbol → identical moniker, text, and digest.
    #[test]
    fn render_is_deterministic() {
        let a = render("ggen://example.org/onto/widgets");
        let b = render("ggen://example.org/onto/widgets");
        assert_eq!(a, b);
    }

    #[test]
    fn wip_virtual_doc_never_claims_actuation_authority() {
        let markdown = generate_ggen_markdown(WIP_CLOSURE_URI);
        assert!(markdown.contains("INTENT_ONLY"));
        assert!(!markdown.contains("Authority: `DO`"));
    }
}
