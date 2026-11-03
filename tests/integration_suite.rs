#![forbid(unsafe_code)]
#![allow(
    clippy::unwrap_used,
    clippy::float_cmp,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::same_item_push
)]

use plumb::{
    audit_repository, calculate_power, evaluate_records, exact_mcnemar_test, generate_lockfile,
    load_and_verify_pairs, paired_bootstrap_ci, paired_sign_flip_permutation_test,
    simulator::{generate_synthetic_dataset, SimulationProfile},
    tost_equivalence_test, verify_lockfile, wald_interval, wilcoxon_signed_rank_test,
    wilson_score_interval, write_pairs_jsonl, CaseSeverity, DiscordanceMatrix, EvaluationObjective,
    PlumbCaseSpec, PlumbRegime, SuiteSpec,
};
use rstest::rstest;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_respect_statistics_wald_margin_80_at_100() {
    // With 100 examples at 80% accuracy, 95% Wald margin is ~ +/-7.84% (~ +/-8 points)
    let ci = wald_interval(80, 100, 0.95);
    assert_eq!(ci.point, 0.80);
    let diff = (ci.margin - 0.0784).abs();
    assert!(
        diff < 0.001,
        "Expected Wald margin ~0.0784 (+/-7.84%), got {:.4}",
        ci.margin
    );
    assert!((ci.lower - 0.7216).abs() < 0.001);
    assert!((ci.upper - 0.8784).abs() < 0.001);
}

#[test]
fn test_wilson_score_interval_boundaries() {
    let ci_zero = wilson_score_interval(0, 100, 0.95);
    assert_eq!(ci_zero.point, 0.0);
    assert!(ci_zero.lower >= 0.0);
    assert!(ci_zero.upper > 0.0 && ci_zero.upper < 0.05);

    let ci_full = wilson_score_interval(100, 100, 0.95);
    assert_eq!(ci_full.point, 1.0);
    assert!(ci_full.upper <= 1.0);
    assert!(ci_full.lower > 0.95 && ci_full.lower < 1.0);
}

#[rstest]
#[case(15, 12, 0.7011, false)] // 80% -> 83% high churn (p = 0.7011)
#[case(3, 0, 0.2500, false)] // 80% -> 83% underpowered 3 gains (p = 0.2500)
#[case(6, 0, 0.03125, true)] // 80% -> 86% surgical monotonic win (p = 0.03125)
#[case(11, 2, 0.02246, true)] // 80% -> 89% robust breakthrough win (p = 0.02246)
fn test_exact_mcnemar_cases(
    #[case] n_gain: usize,
    #[case] n_regression: usize,
    #[case] expected_p: f64,
    #[case] is_significant: bool,
) {
    let res = exact_mcnemar_test(n_gain, n_regression);
    assert!(
        (res.p_value_exact - expected_p).abs() < 0.002,
        "n_gain={}, n_reg={}, expected p={:.4}, got {:.4}",
        n_gain,
        n_regression,
        expected_p,
        res.p_value_exact
    );
    assert_eq!(res.p_value_exact <= 0.05, is_significant);
}

#[test]
fn test_discordance_matrix_variance_reduction() {
    let dm = DiscordanceMatrix::from_counts(78, 9, 11, 2);
    assert_eq!(dm.total, 100);
    assert_eq!(dm.discordant_count, 13);
    assert!((dm.delta - 0.09).abs() < 1e-6);
    assert!(dm.se_paired < dm.se_unpaired);
    assert!(dm.variance_reduction_factor > 1.2);
}

#[test]
fn test_bca_and_percentile_bootstrap() {
    let deltas = vec![0.0; 80];
    let mut mixed = deltas;
    for _ in 0..15 {
        mixed.push(1.0);
    }
    for _ in 0..5 {
        mixed.push(-1.0);
    }
    assert_eq!(mixed.len(), 100);

    let boot = paired_bootstrap_ci(&mixed, 2_000, 42, 0.95);
    assert!((boot.bca_95_ci.point - 0.10).abs() < 1e-6);
    assert!(boot.bca_95_ci.lower > 0.0);
    assert!(boot.bca_95_ci.upper > boot.bca_95_ci.lower);
}

#[test]
fn test_tost_equivalence() {
    let eq = tost_equivalence_test(-0.01, 0.015, 0.05, 0.05);
    assert!(eq.is_equivalent);
    assert!(eq.p_value_tost < 0.05);

    let not_eq = tost_equivalence_test(0.08, 0.02, 0.05, 0.05);
    assert!(!not_eq.is_equivalent);
    assert!(not_eq.p_value_tost > 0.05);
}

#[test]
fn test_power_calculation_paired_advantage() {
    let power = calculate_power(0.80, 0.03, 0.15, 0.05, 0.80, 100);
    assert!(power.required_n_paired < power.required_n_unpaired);
    assert!(power.sample_size_reduction_percent > 30.0);
}

#[test]
fn test_wilcoxon_signed_rank_test() {
    let deltas = vec![0.1, 0.2, 0.15, 0.3, -0.05, 0.25, 0.18, 0.12, 0.22, 0.35];
    let res = wilcoxon_signed_rank_test(&deltas);
    assert!(res.p_value < 0.05);
    assert!(res.statistic_w_pos > res.statistic_w_neg);
}

#[test]
fn test_permutation_sign_flip() {
    let all_positive = vec![1.0; 20];
    let p_pos = paired_sign_flip_permutation_test(&all_positive, 2_000, 42);
    assert!(p_pos < 0.001);

    let balanced = vec![1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
    let p_bal = paired_sign_flip_permutation_test(&balanced, 2_000, 42);
    assert!(p_bal > 0.5);
}

fn setup_fixture_files() -> PathBuf {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("packs/fixtures");
    fs::create_dir_all(&base_dir).unwrap();

    let vsp_path = base_dir.join("vsp_rls_paired_100.jsonl");
    if !vsp_path.exists() {
        let p_vsp = generate_synthetic_dataset(SimulationProfile::RobustBreakthroughWin, 100, 42);
        write_pairs_jsonl(&vsp_path, &p_vsp).unwrap();
    }

    let orch_path = base_dir.join("orchestrator_surgical_100.jsonl");
    if !orch_path.exists() {
        let p_orch = generate_synthetic_dataset(SimulationProfile::SurgicalMonotonicWin, 100, 43);
        write_pairs_jsonl(&orch_path, &p_orch).unwrap();
    }

    let dist_path = base_dir.join("harness_distillation_100.jsonl");
    if !dist_path.exists() {
        let p_dist =
            generate_synthetic_dataset(SimulationProfile::DistillationEquivalence, 100, 44);
        write_pairs_jsonl(&dist_path, &p_dist).unwrap();
    }

    base_dir
}

#[test]
fn test_generate_and_evaluate_synthetic_fixtures() {
    let base_dir = setup_fixture_files();

    // 1. Critical Blocker: Robust win (80% -> 89%, p=0.022)
    let vsp_path = base_dir.join("vsp_rls_paired_100.jsonl");
    let loaded_vsp = load_and_verify_pairs(&vsp_path).unwrap();
    assert_eq!(loaded_vsp.len(), 100);

    let case_vsp = PlumbCaseSpec {
        id: "sf-vsp-rls-paired-win".to_string(),
        stratum: "critical".to_string(),
        severity: CaseSeverity::Blocker,
        description: "VSP test".to_string(),
        pairs_path: "packs/fixtures/vsp_rls_paired_100.jsonl".to_string(),
        objective: EvaluationObjective::Superiority,
        min_examples: 50,
        alpha: 0.05,
        min_delta: 0.0,
        equivalence_margin: 0.05,
        max_regression_rate: 0.05,
        bootstrap_iterations: 2_000,
        seed: 42,
        require_continuous_significance: false,
    };
    let res_vsp = evaluate_records(&case_vsp, &loaded_vsp);
    assert_eq!(
        res_vsp.regime,
        PlumbRegime::ConfirmedStatisticallySignificantWin
    );
    assert!(res_vsp.passed);

    // 4. Counterfactual: High-churn 80% -> 83% noise
    let p_churn = generate_synthetic_dataset(SimulationProfile::HighChurnNoise, 100, 45);
    let churn_path = base_dir.join("negative_80_to_83_churn_100.jsonl");
    write_pairs_jsonl(&churn_path, &p_churn).unwrap();

    let res_churn = evaluate_records(&case_vsp, &p_churn);
    assert!(
        res_churn.regime == PlumbRegime::NoiseWithinMargin
            || res_churn.regime == PlumbRegime::HighRegressionChurnBlocked
    );
    assert!(!res_churn.passed);

    // 5. Counterfactual: Underpowered 80% -> 83% noise
    let p_under = generate_synthetic_dataset(SimulationProfile::Underpowered3Pt, 100, 46);
    let under_path = base_dir.join("negative_80_to_83_underpowered_100.jsonl");
    write_pairs_jsonl(&under_path, &p_under).unwrap();

    let res_under = evaluate_records(&case_vsp, &p_under);
    assert_eq!(res_under.regime, PlumbRegime::UnderpoweredNoiseBlocked);
    assert!(!res_under.passed);

    // 6. Counterfactual: Significant regression (80% -> 68%)
    let p_reg = generate_synthetic_dataset(SimulationProfile::SignificantRegression, 100, 47);
    let reg_path = base_dir.join("negative_significant_regression_100.jsonl");
    write_pairs_jsonl(&reg_path, &p_reg).unwrap();

    let res_reg = evaluate_records(&case_vsp, &p_reg);
    assert_eq!(res_reg.regime, PlumbRegime::SignificantRegressionBlocked);
    assert!(!res_reg.passed);
}

#[test]
fn test_lockfile_generation_and_verification() {
    setup_fixture_files();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let suite_path = manifest_dir.join("packs/softfactory.toml");
    let lock_path = manifest_dir.join("packs/softfactory.lock.json");

    let lock = generate_lockfile(&suite_path, &manifest_dir).unwrap();
    let lock_json = serde_json::to_string_pretty(&lock).unwrap();
    fs::write(&lock_path, lock_json).unwrap();

    let verify_res = verify_lockfile(&lock_path, &suite_path, &manifest_dir);
    assert!(
        verify_res.is_ok(),
        "Verification failed: {:?}",
        verify_res.err()
    );
}

#[test]
fn test_suite_evaluation_end_to_end() {
    setup_fixture_files();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let suite_path = manifest_dir.join("packs/softfactory.toml");
    let suite = SuiteSpec::load_from_file(&suite_path).unwrap();

    let res = plumb::evaluate_suite(&suite, &manifest_dir);
    assert!(
        res.passed,
        "Suite evaluation failed: {}/{} passed",
        res.passed_cases, res.total_cases
    );
    assert_eq!(res.total_cases, 3);
    assert_eq!(res.passed_cases, 3);
    assert_eq!(res.blocker_failures, 0);
    assert!(res.composite_score >= 0.99);
}

#[test]
fn test_repository_hygiene_and_strict_typography() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let report = audit_repository(&manifest_dir);

    for gate in &report.gates {
        assert!(gate.passed, "Gate '{}' failed: {}", gate.name, gate.details);
    }

    assert_eq!(report.passed_gates, 8);
    assert_eq!(report.total_gates, 8);
    assert!(report.passed);
}
