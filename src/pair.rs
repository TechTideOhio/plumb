use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use thiserror::Error;

pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const PAIR_DOMAIN_TAG: &[u8] = b"tideos-plumb-pair-v1\0";

#[derive(Debug, Error)]
pub enum PairError {
    #[error("I/O error reading pairs from {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("JSON error at line {line}: {source}")]
    Json {
        line: usize,
        source: serde_json::Error,
    },
    #[error("Provenance chain broken at record index {index}: expected prev_hash '{expected}', found '{found}'")]
    BrokenChain {
        index: usize,
        expected: String,
        found: String,
    },
    #[error(
        "Invalid record hash at index {index}: calculated '{calculated}', recorded '{recorded}'"
    )]
    InvalidHash {
        index: usize,
        calculated: String,
        recorded: String,
    },
    #[error("Duplicate or mismatched example ID at index {index}: '{example_id}'")]
    MismatchedId { index: usize, example_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PairedRecord {
    pub record_index: usize,
    pub example_id: String,
    pub prompt_sha256: String,
    pub baseline_pass: bool,
    pub candidate_pass: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stratum: Option<String>,
    pub prev_hash: String,
    pub record_hash: String,
}

impl PairedRecord {
    pub fn calculate_hash(
        record_index: usize,
        example_id: &str,
        prompt_sha256: &str,
        baseline_pass: bool,
        candidate_pass: bool,
        baseline_score: Option<f64>,
        candidate_score: Option<f64>,
        stratum: Option<&str>,
        prev_hash: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(PAIR_DOMAIN_TAG);
        hasher.update((record_index as u64).to_be_bytes());
        hasher.update((example_id.len() as u64).to_be_bytes());
        hasher.update(example_id.as_bytes());
        hasher.update(prompt_sha256.as_bytes());
        hasher.update([u8::from(baseline_pass), u8::from(candidate_pass)]);

        if let Some(s) = baseline_score {
            hasher.update(s.to_be_bytes());
        } else {
            hasher.update((-1.0_f64).to_be_bytes());
        }

        if let Some(s) = candidate_score {
            hasher.update(s.to_be_bytes());
        } else {
            hasher.update((-1.0_f64).to_be_bytes());
        }

        if let Some(strat) = stratum {
            hasher.update(strat.as_bytes());
        }

        hasher.update(prev_hash.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn new_chained(
        record_index: usize,
        example_id: impl Into<String>,
        prompt_sha256: impl Into<String>,
        baseline_pass: bool,
        candidate_pass: bool,
        baseline_score: Option<f64>,
        candidate_score: Option<f64>,
        stratum: Option<String>,
        prev_hash: impl Into<String>,
    ) -> Self {
        let ex_id = example_id.into();
        let pr_hash = prompt_sha256.into();
        let prev = prev_hash.into();

        let hash = Self::calculate_hash(
            record_index,
            &ex_id,
            &pr_hash,
            baseline_pass,
            candidate_pass,
            baseline_score,
            candidate_score,
            stratum.as_deref(),
            &prev,
        );

        Self {
            record_index,
            example_id: ex_id,
            prompt_sha256: pr_hash,
            baseline_pass,
            candidate_pass,
            baseline_score,
            candidate_score,
            stratum,
            prev_hash: prev,
            record_hash: hash,
        }
    }

    pub fn is_both_pass(&self) -> bool {
        self.baseline_pass && self.candidate_pass
    }

    pub fn is_both_fail(&self) -> bool {
        !self.baseline_pass && !self.candidate_pass
    }

    pub fn is_gain(&self) -> bool {
        !self.baseline_pass && self.candidate_pass
    }

    pub fn is_regression(&self) -> bool {
        self.baseline_pass && !self.candidate_pass
    }

    pub fn delta(&self) -> f64 {
        let b = if self.candidate_pass { 1.0 } else { 0.0 };
        let a = if self.baseline_pass { 1.0 } else { 0.0 };
        b - a
    }

    pub fn continuous_delta(&self) -> Option<f64> {
        match (self.baseline_score, self.candidate_score) {
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        }
    }
}

pub fn load_and_verify_pairs(path: impl AsRef<Path>) -> Result<Vec<PairedRecord>, PairError> {
    let p = path.as_ref();
    let file = File::open(p).map_err(|e| PairError::Io {
        path: p.display().to_string(),
        source: e,
    })?;

    let reader = BufReader::new(file);
    let mut records = Vec::new();
    let mut expected_prev = GENESIS_HASH.to_string();

    for (line_idx, line_res) in reader.lines().enumerate() {
        let line = line_res.map_err(|e| PairError::Io {
            path: p.display().to_string(),
            source: e,
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let record: PairedRecord = serde_json::from_str(trimmed).map_err(|e| PairError::Json {
            line: line_idx + 1,
            source: e,
        })?;

        if record.prev_hash != expected_prev {
            return Err(PairError::BrokenChain {
                index: record.record_index,
                expected: expected_prev,
                found: record.prev_hash,
            });
        }

        let calculated = PairedRecord::calculate_hash(
            record.record_index,
            &record.example_id,
            &record.prompt_sha256,
            record.baseline_pass,
            record.candidate_pass,
            record.baseline_score,
            record.candidate_score,
            record.stratum.as_deref(),
            &record.prev_hash,
        );

        if calculated != record.record_hash {
            return Err(PairError::InvalidHash {
                index: record.record_index,
                calculated,
                recorded: record.record_hash,
            });
        }

        expected_prev.clone_from(&record.record_hash);
        records.push(record);
    }

    Ok(records)
}

pub fn write_pairs_jsonl(path: impl AsRef<Path>, records: &[PairedRecord]) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    for record in records {
        let serialized = serde_json::to_string(record)?;
        writeln!(file, "{serialized}")?;
    }
    file.flush()
}
