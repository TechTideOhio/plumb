use crate::pair::{PairedRecord, GENESIS_HASH};
use crate::stats::SplitMix64;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationProfile {
    HighChurnNoise,
    Underpowered3Pt,
    SurgicalMonotonicWin,
    RobustBreakthroughWin,
    DistillationEquivalence,
    SignificantRegression,
}

pub fn generate_synthetic_dataset(
    profile: SimulationProfile,
    total_examples: usize,
    seed: u64,
) -> Vec<PairedRecord> {
    let mut rng = SplitMix64::new(seed);
    let mut pairs = Vec::with_capacity(total_examples);
    let mut prev_hash = GENESIS_HASH.to_string();

    let (target_n11, target_n00, target_n01, target_n10) = match profile {
        // 80% -> 83% with 15 gains and 12 regressions (net +3%, high churn, p=0.701)
        SimulationProfile::HighChurnNoise => {
            let n10 = ((total_examples as f64) * 0.12).round() as usize;
            let n01 = ((total_examples as f64) * 0.15).round() as usize;
            let n11 = ((total_examples as f64) * 0.68).round() as usize;
            let n00 = total_examples.saturating_sub(n11 + n01 + n10);
            (n11, n00, n01, n10)
        }
        // 80% -> 83% with 3 gains and 0 regressions (net +3%, n_d = 3 < 6, p=0.25)
        SimulationProfile::Underpowered3Pt => {
            let n10 = 0;
            let n01 = ((total_examples as f64) * 0.03).round().max(1.0) as usize;
            let n11 = ((total_examples as f64) * 0.80).round() as usize;
            let n00 = total_examples.saturating_sub(n11 + n01 + n10);
            (n11, n00, n01, n10)
        }
        // 80% -> 86% with 6 gains and 0 regressions (net +6%, surgical monotonic win, p=0.031)
        SimulationProfile::SurgicalMonotonicWin => {
            let n10 = 0;
            let n01 = ((total_examples as f64) * 0.06).round().max(6.0) as usize;
            let n11 = ((total_examples as f64) * 0.80).round() as usize;
            let n00 = total_examples.saturating_sub(n11 + n01 + n10);
            (n11, n00, n01, n10)
        }
        // 80% -> 89% with 11 gains and 2 regressions (net +9%, robust win, p=0.022)
        SimulationProfile::RobustBreakthroughWin => {
            let n10 = ((total_examples as f64) * 0.02).round().max(2.0) as usize;
            let n01 = ((total_examples as f64) * 0.11).round().max(11.0) as usize;
            let n11 = ((total_examples as f64) * 0.78).round() as usize;
            let n00 = total_examples.saturating_sub(n11 + n01 + n10);
            (n11, n00, n01, n10)
        }
        // 82% -> 81% with 1 gain and 2 regressions (net -1%, TOST equivalent within +/-5%)
        SimulationProfile::DistillationEquivalence => {
            let n10 = ((total_examples as f64) * 0.02).round().max(2.0) as usize;
            let n01 = ((total_examples as f64) * 0.01).round().max(1.0) as usize;
            let n11 = ((total_examples as f64) * 0.80).round() as usize;
            let n00 = total_examples.saturating_sub(n11 + n01 + n10);
            (n11, n00, n01, n10)
        }
        // 80% -> 68% with 2 gains and 14 regressions (net -12%, significant regression, p=0.004)
        SimulationProfile::SignificantRegression => {
            let n10 = ((total_examples as f64) * 0.14).round().max(14.0) as usize;
            let n01 = ((total_examples as f64) * 0.02).round().max(2.0) as usize;
            let n11 = ((total_examples as f64) * 0.66).round() as usize;
            let n00 = total_examples.saturating_sub(n11 + n01 + n10);
            (n11, n00, n01, n10)
        }
    };

    let mut outcomes = Vec::with_capacity(total_examples);
    for _ in 0..target_n11 {
        outcomes.push((true, true));
    }
    for _ in 0..target_n00 {
        outcomes.push((false, false));
    }
    for _ in 0..target_n01 {
        outcomes.push((false, true));
    }
    for _ in 0..target_n10 {
        outcomes.push((true, false));
    }

    while outcomes.len() < total_examples {
        outcomes.push((true, true));
    }

    // Shuffle outcomes deterministically
    for i in (1..outcomes.len()).rev() {
        let j = rng.gen_range(i + 1);
        outcomes.swap(i, j);
    }

    for (idx, &(base_p, cand_p)) in outcomes.iter().enumerate().take(total_examples) {
        let base_score = if base_p {
            0.80 + rng.next_f64() * 0.20
        } else {
            rng.next_f64() * 0.50
        };
        let cand_score = if cand_p {
            0.82 + rng.next_f64() * 0.18
        } else {
            rng.next_f64() * 0.48
        };

        let prompt_seed = format!("prompt-hash-seed-{idx}-{seed}");
        let mut hasher = Sha256::new();
        hasher.update(prompt_seed.as_bytes());
        let prompt_sha256 = format!("{:x}", hasher.finalize());

        let record = PairedRecord::new_chained(
            idx,
            format!("ex-{idx:04}"),
            prompt_sha256,
            base_p,
            cand_p,
            Some((base_score * 100.0).round() / 100.0),
            Some((cand_score * 100.0).round() / 100.0),
            Some("synthetic".to_string()),
            prev_hash,
        );

        prev_hash = record.record_hash.clone();
        pairs.push(record);
    }

    pairs
}
