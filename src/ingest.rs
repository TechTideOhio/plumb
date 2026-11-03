use crate::pair::{PairedRecord, GENESIS_HASH};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub fn ingest_paired_harness_runs(
    runs_dir: impl AsRef<Path>,
    baseline_agent: &str,
    candidate_agent: &str,
) -> Result<Vec<PairedRecord>> {
    let p = runs_dir.as_ref();
    if !p.exists() {
        return Ok(Vec::new());
    }

    let mut baseline_runs = Vec::new();
    let mut candidate_runs = Vec::new();

    for entry in fs::read_dir(p).context("failed to read harness runs directory")? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
        if dir_name.contains(baseline_agent) {
            baseline_runs.push(path);
        } else if dir_name.contains(candidate_agent) {
            candidate_runs.push(path);
        }
    }

    baseline_runs.sort();
    candidate_runs.sort();

    let mut pairs = Vec::new();
    let min_len = baseline_runs.len().min(candidate_runs.len());
    let mut prev_hash = GENESIS_HASH.to_string();

    for i in 0..min_len {
        let path_a = &baseline_runs[i];
        let path_b = &candidate_runs[i];

        let prompt_path = path_a.join("prompt.md");
        let prompt = if prompt_path.exists() {
            fs::read_to_string(prompt_path).unwrap_or_default()
        } else {
            format!("Harness Task {i}")
        };

        let mut hasher = Sha256::new();
        hasher.update(prompt.as_bytes());
        let prompt_sha256 = format!("{:x}", hasher.finalize());

        let res_a_path = path_a.join("result.json");
        let base_pass = if res_a_path.exists() {
            let content = fs::read_to_string(res_a_path).unwrap_or_default();
            content.contains("\"success\"")
        } else {
            false
        };

        let res_b_path = path_b.join("result.json");
        let cand_pass = if res_b_path.exists() {
            let content = fs::read_to_string(res_b_path).unwrap_or_default();
            content.contains("\"success\"")
        } else {
            false
        };

        let record = PairedRecord::new_chained(
            i,
            format!("task-{i:04}"),
            prompt_sha256,
            base_pass,
            cand_pass,
            None,
            None,
            Some("agent-harness".to_string()),
            prev_hash,
        );

        prev_hash = record.record_hash.clone();
        pairs.push(record);
    }

    Ok(pairs)
}
