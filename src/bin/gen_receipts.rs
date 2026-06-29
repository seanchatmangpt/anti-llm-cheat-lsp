//! Receipt generator for anti-llm-cheat-lsp.
//!
//! For every transcript under `transcripts/`, emit a receipt under `receipts/`
//! carrying a real BLAKE3 digest of the transcript bytes, boundary/checkpoint
//! markers, the verifiable raw command, the negative-control reference, and a
//! bounded status. The notebook family carries a refusal receipt (no transcript;
//! the digest attests the canonical refusal statement) so the delta matrix's
//! `REFUSED_BY_LAW_WITH_RECEIPT` claim is backed by an artifact on disk.
//!
//! Run from the example directory:
//!   cargo run -p anti-llm-cheat-lsp --bin gen_receipts

use std::{fs, path::Path};

const NOTEBOOK_REFUSAL_STATEMENT: &str =
    "notebookDocumentSync capability is not advertised; notebook document \
     synchronization is REFUSED by law in anti-llm-cheat-lsp.";

fn write_receipt(
    receipts_dir: &Path,
    receipt_name: &str,
    digest: &str,
    raw_command: &str,
    negative_control: &str,
    status: &str,
) {
    let body = format!(
        "{{\n  \"digest_algorithm\": \"BLAKE3\",\n  \"digest\": \"{}\",\n  \
         \"boundary\": \"-----BEGIN RECEIPT-----\",\n  \
         \"checkpoint\": \"-----END RECEIPT-----\",\n  \
         \"raw_command\": \"{}\",\n  \"negative_control\": \"{}\",\n  \
         \"status\": \"{}\"\n}}\n",
        digest, raw_command, negative_control, status
    );
    let path = receipts_dir.join(receipt_name);
    fs::write(&path, body).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn main() {
    let example_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let transcripts_dir = example_dir.join("transcripts");
    let receipts_dir = example_dir.join("receipts");
    fs::create_dir_all(&receipts_dir).expect("create receipts dir");

    let mut count = 0usize;
    let mut entries: Vec<_> = fs::read_dir(&transcripts_dir)
        .expect("read transcripts dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("jsonl"))
        .collect();
    entries.sort();

    for transcript in entries {
        let stem = transcript
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix("_positive.jsonl"))
            .map(|s| s.to_string());
        let Some(stem) = stem else { continue };

        let bytes = fs::read(&transcript).expect("read transcript");
        let digest = blake3::hash(&bytes).to_hex().to_string();
        let receipt_name = format!("{stem}_receipt.json");
        let raw_command = format!("blake3 transcripts/{stem}_positive.jsonl");
        let negative_control = format!("fixtures/negative_controls/{stem}");

        write_receipt(
            &receipts_dir,
            &receipt_name,
            &digest,
            &raw_command,
            &negative_control,
            "ADMITTED",
        );
        // Emit the digest map so the delta-matrix source can be synced.
        println!("{stem} {digest}");
        count += 1;
    }

    // Notebook refusal receipt — no transcript; digest attests the refusal text.
    let refusal_digest = blake3::hash(NOTEBOOK_REFUSAL_STATEMENT.as_bytes()).to_hex().to_string();
    write_receipt(
        &receipts_dir,
        "notebook_refusal_receipt.json",
        &refusal_digest,
        "blake3 <<< notebook-refusal-statement",
        "fixtures/negative_controls/notebook_out_of_scope",
        "REFUSED",
    );
    println!("notebook_refusal {refusal_digest}");

    eprintln!("wrote {} transcript receipts + 1 refusal receipt", count);
}
