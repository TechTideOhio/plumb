use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalEconomics {
    pub baseline_cost_per_eval: f64,
    pub candidate_cost_per_eval: f64,
    pub sample_size_n: usize,
    pub unpaired_equivalent_n: usize,
    pub token_cost_saved_usd: f64,
    pub regression_risk_cost_usd: f64,
    pub net_economic_value_usd: f64,
}

impl StatisticalEconomics {
    pub fn calculate(
        sample_size_n: usize,
        variance_reduction_factor: f64,
        cost_per_example_usd: f64,
        false_ship_incident_cost_usd: f64,
        detected_regression_rate: f64,
    ) -> Self {
        let vrf = variance_reduction_factor.max(1.0);
        let unpaired_eq_n = ((sample_size_n as f64) * vrf).ceil() as usize;
        let saved_examples = (unpaired_eq_n.saturating_sub(sample_size_n)) as f64;
        let token_cost_saved = saved_examples * cost_per_example_usd * 2.0;

        let regression_prevented_value = if detected_regression_rate > 0.05 {
            false_ship_incident_cost_usd
        } else {
            0.0
        };

        let net_value = token_cost_saved + regression_prevented_value;

        Self {
            baseline_cost_per_eval: cost_per_example_usd,
            candidate_cost_per_eval: cost_per_example_usd,
            sample_size_n,
            unpaired_equivalent_n: unpaired_eq_n,
            token_cost_saved_usd: token_cost_saved,
            regression_risk_cost_usd: false_ship_incident_cost_usd,
            net_economic_value_usd: net_value,
        }
    }
}
