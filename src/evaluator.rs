use crate::pair::{load_and_verify_pairs, PairedRecord};
use crate::spec::{CaseSeverity, EvaluationObjective, PlumbCaseSpec, SuiteSpec};
use crate::stats::{
    benjamini_hochberg_adjust, calculate_power, exact_mcnemar_test, holm_bonferroni_adjust,
    paired_bootstrap_ci, paired_sign_flip_permutation_test, tost_equivalence_test, wald_interval,
    wilcoxon_signed_rank_test, wilson_score_interval, BootstrapResult, ConfidenceInterval,
    DiscordanceMatrix, McNemarResult, PowerCalculation, TostResult, WilcoxonResult,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlumbRegime {
    ConfirmedStatisticallySignificantWin,
    StatisticallyEquivalentWithinMargin,
    NoiseWithinMargin,
    UnderpoweredNoiseBlocked,
    HighRegressionChurnBlocked,
    SignificantRegressionBlocked,
    UnpairedExampleMismatchBlocked,
    ProvenanceChainBrokenBlocked,
    InsufficientExamplesBlocked,
}

impl PlumbRegime {
    pub fn is_pass(&self) -> bool {
        matches!(
            self,
            Self::ConfirmedStatisticallySignificantWin | Self::StatisticallyEquivalentWithinMargin
        )
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::ConfirmedStatisticallySignificantWin => {
                "Confirmed statistically significant win on identical benchmark examples"
            }
            Self::StatisticallyEquivalentWithinMargin => {
                "Confirmed statistically equivalent within specified tolerance margin (TOST)"
            }
            Self::NoiseWithinMargin => {
                "Observed delta is statistical noise: 95% confidence interval spans zero"
            }
            Self::UnderpoweredNoiseBlocked => {
                "Evaluation underpowered: insufficient discordant examples to establish significance"
            }
            Self::HighRegressionChurnBlocked => {
                "High churn detected: candidate broke too many previously passing examples"
            }
            Self::SignificantRegressionBlocked => {
                "Candidate model or prompt exhibited statistically significant regression"
            }
            Self::UnpairedExampleMismatchBlocked => {
                "Example IDs or prompt hashes mismatched between baseline and candidate"
            }
            Self::ProvenanceChainBrokenBlocked => {
                "Sequential SHA-256 Merkle provenance hash chain is broken or tampered"
            }
            Self::InsufficientExamplesBlocked => {
                "Fewer benchmark examples than minimum required threshold"
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseEvaluationResult {
    pub case_id: String,
    pub stratum: String,
    pub severity: CaseSeverity,
    pub objective: EvaluationObjective,
    pub regime: PlumbRegime,
    pub passed: bool,
    pub total_examples: usize,
    pub baseline_rate: f64,
    pub candidate_rate: f64,
    pub observed_delta: f64,
    pub regression_rate: f64,
    pub baseline_wald_ci: ConfidenceInterval,
    pub baseline_wilson_ci: ConfidenceInterval,
    pub candidate_wald_ci: ConfidenceInterval,
    pub candidate_wilson_ci: ConfidenceInterval,
    pub discordance: DiscordanceMatrix,
    pub mcnemar: McNemarResult,
    pub bootstrap: BootstrapResult,
    pub permutation_p_value: f64,
    pub wilcoxon: Option<WilcoxonResult>,
    pub tost: Option<TostResult>,
    pub power: PowerCalculation,
    pub raw_p_value: f64,
    pub bh_adjusted_q_value: f64,
    pub holm_adjusted_p_value: f64,
    pub failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratumScore {
    pub stratum: String,
    pub weight: f64,
    pub total_cases: usize,
    pub passed_cases: usize,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteEvaluationResult {
    pub suite_name: String,
    pub suite_version: String,
    pub passed: bool,
    pub total_cases: usize,
    pub passed_cases: usize,
    pub blocker_failures: usize,
    pub composite_score: f64,
    pub strata: BTreeMap<String, StratumScore>,
    pub cases: Vec<CaseEvaluationResult>,
}

pub fn evaluate_case(case: &PlumbCaseSpec, root_dir: impl AsRef<Path>) -> CaseEvaluationResult {
    let dataset_path = root_dir.as_ref().join(&case.pairs_path);
    let records = match load_and_verify_pairs(&dataset_path) {
        Ok(r) => r,
        Err(err) => {
            let regime = match err {
                crate::pair::PairError::BrokenChain { .. }
                | crate::pair::PairError::InvalidHash { .. } => {
                    PlumbRegime::ProvenanceChainBrokenBlocked
                }
                crate::pair::PairError::MismatchedId { .. } => {
                    PlumbRegime::UnpairedExampleMismatchBlocked
                }
                _ => PlumbRegime::InsufficientExamplesBlocked,
            };

            let zero_ci = ConfidenceInterval {
                point: 0.0,
                lower: 0.0,
                upper: 0.0,
                margin: 0.0,
                confidence_level: 0.95,
            };

            return CaseEvaluationResult {
                case_id: case.id.clone(),
                stratum: case.stratum.clone(),
                severity: case.severity,
                objective: case.objective,
                regime,
                passed: false,
                total_examples: 0,
                baseline_rate: 0.0,
                candidate_rate: 0.0,
                observed_delta: 0.0,
                regression_rate: 0.0,
                baseline_wald_ci: zero_ci.clone(),
                baseline_wilson_ci: zero_ci.clone(),
                candidate_wald_ci: zero_ci.clone(),
                candidate_wilson_ci: zero_ci.clone(),
                discordance: DiscordanceMatrix::from_counts(0, 0, 0, 0),
                mcnemar: exact_mcnemar_test(0, 0),
                bootstrap: BootstrapResult {
                    delta_mean: 0.0,
                    delta_median: 0.0,
                    se_bootstrap: 0.0,
                    percentile_95_ci: zero_ci.clone(),
                    bca_95_ci: zero_ci,
                    iterations: case.bootstrap_iterations,
                    seed: case.seed,
                },
                permutation_p_value: 1.0,
                wilcoxon: None,
                tost: None,
                power: calculate_power(0.5, 0.05, 0.1, case.alpha, 0.8, 0),
                raw_p_value: 1.0,
                bh_adjusted_q_value: 1.0,
                holm_adjusted_p_value: 1.0,
                failure_reasons: vec![format!("Failed to load dataset: {err}")],
            };
        }
    };

    evaluate_records(case, &records)
}

pub fn evaluate_records(case: &PlumbCaseSpec, records: &[PairedRecord]) -> CaseEvaluationResult {
    let total = records.len();
    let mut failure_reasons = Vec::new();

    if total < case.min_examples {
        failure_reasons.push(format!(
            "Insufficient examples: got {total}, requires at least {}",
            case.min_examples
        ));
    }

    let mut n_11 = 0;
    let mut n_00 = 0;
    let mut n_01 = 0;
    let mut n_10 = 0;
    let mut deltas = Vec::with_capacity(total);
    let mut continuous_deltas = Vec::new();

    for r in records {
        if r.is_both_pass() {
            n_11 += 1;
        } else if r.is_both_fail() {
            n_00 += 1;
        } else if r.is_gain() {
            n_01 += 1;
        } else if r.is_regression() {
            n_10 += 1;
        }
        deltas.push(r.delta());
        if let Some(cd) = r.continuous_delta() {
            continuous_deltas.push(cd);
        }
    }

    let discordance = DiscordanceMatrix::from_counts(n_11, n_00, n_01, n_10);
    let baseline_passes = n_11 + n_10;
    let candidate_passes = n_11 + n_01;

    let base_wald = wald_interval(baseline_passes, total, 0.95);
    let base_wilson = wilson_score_interval(baseline_passes, total, 0.95);
    let cand_wald = wald_interval(candidate_passes, total, 0.95);
    let cand_wilson = wilson_score_interval(candidate_passes, total, 0.95);

    let mcnemar = exact_mcnemar_test(n_01, n_10);
    let bootstrap = paired_bootstrap_ci(&deltas, case.bootstrap_iterations, case.seed, 0.95);
    let permutation_p = paired_sign_flip_permutation_test(&deltas, 5_000, case.seed + 1);

    let wilcoxon = if !continuous_deltas.is_empty() {
        Some(wilcoxon_signed_rank_test(&continuous_deltas))
    } else {
        None
    };

    let tost = if case.objective == EvaluationObjective::Equivalence {
        Some(tost_equivalence_test(
            discordance.delta,
            discordance.se_paired,
            case.equivalence_margin,
            case.alpha,
        ))
    } else {
        None
    };

    let power = calculate_power(
        discordance.baseline_rate,
        if discordance.delta.abs() > 0.005 {
            discordance.delta.abs()
        } else {
            0.03
        },
        discordance.discordance_rate.max(0.05),
        case.alpha,
        0.80,
        total,
    );

    let regression_rate = if total > 0 {
        (n_10 as f64) / (total as f64)
    } else {
        0.0
    };

    let raw_p_value = if case.objective == EvaluationObjective::Equivalence {
        tost.as_ref().map_or(1.0, |t| t.p_value_tost)
    } else {
        mcnemar.p_value_exact
    };

    // Determine statistical regime
    let regime = if total < case.min_examples {
        PlumbRegime::InsufficientExamplesBlocked
    } else if discordance.delta < 0.0
        && (bootstrap.bca_95_ci.upper < 0.0 || mcnemar.p_value_exact <= case.alpha)
    {
        failure_reasons.push(format!(
            "Significant regression detected: delta {:.2}% with 95% CI upper bound {:.2}%",
            discordance.delta * 100.0,
            bootstrap.bca_95_ci.upper * 100.0
        ));
        PlumbRegime::SignificantRegressionBlocked
    } else if regression_rate > case.max_regression_rate {
        failure_reasons.push(format!(
            "Candidate regression rate {:.2}% exceeded maximum allowable limit {:.2}%",
            regression_rate * 100.0,
            case.max_regression_rate * 100.0
        ));
        PlumbRegime::HighRegressionChurnBlocked
    } else {
        match case.objective {
            EvaluationObjective::Superiority => {
                if discordance.delta < 0.0 && bootstrap.bca_95_ci.upper < 0.0 {
                    failure_reasons.push(format!(
                        "Significant regression detected: delta {:.2}% with 95% CI upper bound {:.2}%",
                        discordance.delta * 100.0,
                        bootstrap.bca_95_ci.upper * 100.0
                    ));
                    PlumbRegime::SignificantRegressionBlocked
                } else if discordance.discordant_count < 6 {
                    failure_reasons.push(format!(
                        "Underpowered evaluation: discordant count n_d = {} < 6 (cannot reject null at alpha = 0.05)",
                        discordance.discordant_count
                    ));
                    PlumbRegime::UnderpoweredNoiseBlocked
                } else if bootstrap.bca_95_ci.lower > case.min_delta
                    && mcnemar.p_value_exact <= case.alpha
                {
                    PlumbRegime::ConfirmedStatisticallySignificantWin
                } else {
                    failure_reasons.push(format!(
                        "Observed delta {:.2}% is statistical noise (exact p = {:.4} > {:.4}, BCa 95% CI [{:.2}%, {:.2}%] spans zero/min_delta)",
                        discordance.delta * 100.0,
                        mcnemar.p_value_exact,
                        case.alpha,
                        bootstrap.bca_95_ci.lower * 100.0,
                        bootstrap.bca_95_ci.upper * 100.0
                    ));
                    PlumbRegime::NoiseWithinMargin
                }
            }
            EvaluationObjective::Equivalence => {
                let eq = tost.as_ref().is_some_and(|t| t.is_equivalent);
                if eq {
                    PlumbRegime::StatisticallyEquivalentWithinMargin
                } else {
                    failure_reasons.push(format!(
                        "Candidate failed equivalence test within margin +/-{:.2}% (TOST p = {:.4} > {:.4})",
                        case.equivalence_margin * 100.0,
                        tost.as_ref().map_or(1.0, |t| t.p_value_tost),
                        case.alpha
                    ));
                    PlumbRegime::NoiseWithinMargin
                }
            }
            EvaluationObjective::NonInferiority => {
                let non_inferior_margin = -case.equivalence_margin.abs();
                if bootstrap.bca_95_ci.lower >= non_inferior_margin {
                    PlumbRegime::ConfirmedStatisticallySignificantWin
                } else {
                    failure_reasons.push(format!(
                        "Non-inferiority bound breached: BCa 95% CI lower bound {:.2}% < margin {:.2}%",
                        bootstrap.bca_95_ci.lower * 100.0,
                        non_inferior_margin * 100.0
                    ));
                    PlumbRegime::SignificantRegressionBlocked
                }
            }
        }
    };

    let passed = regime.is_pass() && failure_reasons.is_empty();

    CaseEvaluationResult {
        case_id: case.id.clone(),
        stratum: case.stratum.clone(),
        severity: case.severity,
        objective: case.objective,
        regime,
        passed,
        total_examples: total,
        baseline_rate: discordance.baseline_rate,
        candidate_rate: discordance.candidate_rate,
        observed_delta: discordance.delta,
        regression_rate,
        baseline_wald_ci: base_wald,
        baseline_wilson_ci: base_wilson,
        candidate_wald_ci: cand_wald,
        candidate_wilson_ci: cand_wilson,
        discordance,
        mcnemar,
        bootstrap,
        permutation_p_value: permutation_p,
        wilcoxon,
        tost,
        power,
        raw_p_value,
        bh_adjusted_q_value: raw_p_value,
        holm_adjusted_p_value: raw_p_value,
        failure_reasons,
    }
}

pub fn evaluate_suite(suite: &SuiteSpec, root_dir: impl AsRef<Path>) -> SuiteEvaluationResult {
    let root = root_dir.as_ref();
    let mut cases = Vec::with_capacity(suite.cases.len());

    for case_spec in &suite.cases {
        let res = evaluate_case(case_spec, root);
        cases.push(res);
    }

    // Apply FDR and FWER adjustments across all cases in the suite
    let raw_p_values: Vec<f64> = cases.iter().map(|c| c.raw_p_value).collect();
    let bh_q_values = benjamini_hochberg_adjust(&raw_p_values);
    let holm_p_values = holm_bonferroni_adjust(&raw_p_values);

    for (i, case) in cases.iter_mut().enumerate() {
        case.bh_adjusted_q_value = bh_q_values[i];
        case.holm_adjusted_p_value = holm_p_values[i];
    }

    let mut stratum_counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut blocker_failures = 0;

    for case in &cases {
        let entry = stratum_counts.entry(case.stratum.clone()).or_insert((0, 0));
        entry.0 += 1;
        if case.passed {
            entry.1 += 1;
        } else if case.severity == CaseSeverity::Blocker {
            blocker_failures += 1;
        }
    }

    let mut strata = BTreeMap::new();
    let mut composite_score = 0.0;

    for (stratum_name, (total_c, passed_c)) in stratum_counts {
        let weight = match stratum_name.as_str() {
            "critical" => suite.weights.critical,
            "core" => suite.weights.core,
            "hygiene" => suite.weights.hygiene,
            _ => 0.0,
        };

        let pass_rate = if total_c > 0 {
            (passed_c as f64) / (total_c as f64)
        } else {
            0.0
        };

        composite_score += weight * pass_rate;

        strata.insert(
            stratum_name.clone(),
            StratumScore {
                stratum: stratum_name,
                weight,
                total_cases: total_c,
                passed_cases: passed_c,
                pass_rate,
            },
        );
    }

    let total_cases = cases.len();
    let passed_cases = cases.iter().filter(|c| c.passed).count();
    let passed = blocker_failures == 0 && passed_cases == total_cases;

    SuiteEvaluationResult {
        suite_name: suite.suite.name.clone(),
        suite_version: suite.suite.version.clone(),
        passed,
        total_cases,
        passed_cases,
        blocker_failures,
        composite_score,
        strata,
        cases,
    }
}
