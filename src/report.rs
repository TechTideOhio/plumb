use crate::evaluator::{CaseEvaluationResult, SuiteEvaluationResult};
use crate::spec::SuiteSpec;
use serde_json::json;

pub fn render_physics_table() -> String {
    let mut out = String::new();
    out.push_str("\n+----------------------------------------------------------------------------------------------------------------------+\n");
    out.push_str("|                     RESPECT STATISTICS: PAIRED EVALUATION DISTORTION & NOISE MATRIX (plumb Tool 11)                  |\n");
    out.push_str("+----------------------------------------------------------------------------------------------------------------------+\n");
    out.push_str("| Baseline: 100 examples at 80.0% accuracy -> 95% Wald Margin: +/-7.84% (~ +/-8 pts). A jump to 83% is probably noise.|\n");
    out.push_str("+----------------------------------+-------+--------------------+---------------------+--------------------+-----------+\n");
    out.push_str("| Evaluation Scenario              | Delta | Discordant (Gain/R)| Unpaired 95% CI     | Paired Boot 95% CI | McNemar p | Diagnosis |\n");
    out.push_str("+----------------------------------+-------+--------------------+---------------------+--------------------+-----------+\n");
    out.push_str("| 1. 80% -> 83% High-Churn Mirage  | +3.0% | 27 pairs (15G, 12R)| [-7.84%, +13.84%]   | [-7.00%, +13.00%]  | p = 0.701 | NOISE     |\n");
    out.push_str("| 2. 80% -> 83% Low-Sample Noise   | +3.0% |  3 pairs ( 3G,  0R)| [-7.84%, +13.84%]   | [ 0.00%,  +7.00%]  | p = 0.250 | NOISE     |\n");
    out.push_str("| 3. 80% -> 86% Surgical Real Win  | +6.0% |  6 pairs ( 6G,  0R)| [-4.40%, +16.40%]*  | [+2.00%, +11.00%]  | p = 0.031 | WIN (0 R) |\n");
    out.push_str("| 4. 80% -> 89% Robust Breakthrough| +9.0% | 13 pairs (11G,  2R)| [-1.00%, +19.00%]*  | [+2.00%, +16.00%]  | p = 0.023 | SOLID WIN |\n");
    out.push_str("| 5. 80% -> 79% Model Distillation | -1.0% |  3 pairs ( 1G,  2R)| [-12.2%, +10.20%]   | [-4.00%,  +2.00%]  | TOST<0.01 | EQUIVALENT|\n");
    out.push_str("+----------------------------------+-------+--------------------+---------------------+--------------------+-----------+\n");
    out.push_str("| * Notice: Unpaired tests miss Scenarios 3 & 4 because single-arm variance swamps the +6% and +9% deltas.             |\n");
    out.push_str("| Paired testing subtracts common-mode benchmark difficulty and evaluates only discordant examples.                    |\n");
    out.push_str("+----------------------------------------------------------------------------------------------------------------------+\n\n");
    out
}

pub fn export_mermaid_flowchart(reports: &[CaseEvaluationResult]) -> String {
    let mut chart = String::from("flowchart LR\n");
    chart.push_str("    subgraph BenchmarkInputs [Identical Benchmark Examples]\n");
    chart.push_str("        EX[N Examples with Hash Verification]\n");
    chart.push_str("    end\n");
    chart.push_str("    EX --> DM[2x2 Discordance Matrix: Gains vs Regressions]\n");
    chart.push_str("    subgraph StatisticalTests [Paired Inference Gates]\n");
    chart.push_str("        DM --> MC[Exact Binomial McNemar Test]\n");
    chart.push_str("        DM --> BS[Seeded SplitMix64 Bootstrap 95% CI]\n");
    chart.push_str("        DM --> TOST[TOST Equivalence & Non-Inferiority]\n");
    chart.push_str("    end\n");
    chart.push_str("    subgraph Verdicts [Decision Envelopes]\n");
    chart.push_str("        MC --> V[Confirmed Win or Noise Gated]\n");
    chart.push_str("        BS --> V\n");
    chart.push_str("        TOST --> V\n");
    chart.push_str("    end\n");

    for r in reports {
        let node_id = r.case_id.replace('-', "_");
        chart.push_str(&format!(
            "    V -.-> {node_id}[\"{}: Delta {:+.1}%, p={:.4}\"]\n",
            r.case_id,
            r.observed_delta * 100.0,
            r.raw_p_value
        ));
    }

    chart
}

pub fn export_otel_spans(
    suite_res: &SuiteEvaluationResult,
    suite: &SuiteSpec,
) -> serde_json::Value {
    let spans: Vec<serde_json::Value> = suite_res
        .cases
        .iter()
        .map(|r| {
            json!({
                "trace_id": format!("tideos-plumb-{}", suite.suite.name),
                "span_id": format!("case-{}", r.case_id),
                "name": "eval.statistical.significance",
                "attributes": {
                    "eval.case_id": r.case_id,
                    "eval.stratum": r.stratum,
                    "eval.regime": format!("{:?}", r.regime),
                    "eval.passed": r.passed,
                    "eval.total_examples": r.total_examples,
                    "eval.baseline_rate": r.baseline_rate,
                    "eval.candidate_rate": r.candidate_rate,
                    "eval.observed_delta": r.observed_delta,
                    "eval.regression_rate": r.regression_rate,
                    "eval.p_value_mcnemar": r.mcnemar.p_value_exact,
                    "eval.bca_ci_lower": r.bootstrap.bca_95_ci.lower,
                    "eval.bca_ci_upper": r.bootstrap.bca_95_ci.upper,
                    "eval.bh_adjusted_q_value": r.bh_adjusted_q_value,
                    "eval.variance_reduction_factor": r.discordance.variance_reduction_factor,
                }
            })
        })
        .collect();

    json!({
        "resource": {
            "service.name": "tideos-eval-kit",
            "service.version": "0.1.0",
            "suite.name": suite.suite.name
        },
        "spans": spans
    })
}

pub fn export_reval_ratchet(suite_res: &SuiteEvaluationResult) -> serde_json::Value {
    let cases: Vec<serde_json::Value> = suite_res
        .cases
        .iter()
        .map(|c| {
            json!({
                "case_id": c.case_id,
                "baseline_rate": c.baseline_rate,
                "target_delta": c.observed_delta.max(0.0),
                "p_value_max": c.raw_p_value,
                "bca_ci_lower_bound": c.bootstrap.bca_95_ci.lower,
                "status": if c.passed { "pass" } else { "fail" }
            })
        })
        .collect();

    json!({
        "ratchet_version": "v1",
        "tool": "plumb",
        "composite_score": suite_res.composite_score,
        "cases": cases
    })
}
