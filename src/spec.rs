use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpecError {
    #[error("failed to read suite specification from {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse TOML suite specification: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("strata weights must sum to 1.00 (got {0:.4})")]
    InvalidWeights(f64),
    #[error("case '{0}' min_examples must be at least 10 (got {1})")]
    InsufficientMinExamples(String, usize),
    #[error("case '{0}' alpha must be in range (0.0, 0.5) (got {1:.4})")]
    InvalidAlpha(String, f64),
    #[error("case '{0}' max_regression_rate must be in range [0.0, 1.0] (got {1:.4})")]
    InvalidRegressionRate(String, f64),
    #[error("case '{0}' not found in suite")]
    CaseNotFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseSeverity {
    Blocker,
    Graded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationObjective {
    Superiority,
    Equivalence,
    NonInferiority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlumbCaseSpec {
    pub id: String,
    pub stratum: String,
    pub severity: CaseSeverity,
    pub description: String,
    pub pairs_path: String,
    pub objective: EvaluationObjective,
    #[serde(default = "default_min_examples")]
    pub min_examples: usize,
    #[serde(default = "default_alpha")]
    pub alpha: f64,
    #[serde(default = "default_min_delta")]
    pub min_delta: f64,
    #[serde(default = "default_equivalence_margin")]
    pub equivalence_margin: f64,
    #[serde(default = "default_max_regression_rate")]
    pub max_regression_rate: f64,
    #[serde(default = "default_bootstrap_iterations")]
    pub bootstrap_iterations: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default)]
    pub require_continuous_significance: bool,
}

fn default_min_examples() -> usize {
    30
}

fn default_alpha() -> f64 {
    0.05
}

fn default_min_delta() -> f64 {
    0.0
}

fn default_equivalence_margin() -> f64 {
    0.05
}

fn default_max_regression_rate() -> f64 {
    0.05
}

fn default_bootstrap_iterations() -> usize {
    10_000
}

fn default_seed() -> u64 {
    42
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrataWeights {
    pub critical: f64,
    pub core: f64,
    pub hygiene: f64,
}

impl Default for StrataWeights {
    fn default() -> Self {
        Self {
            critical: 0.50,
            core: 0.35,
            hygiene: 0.15,
        }
    }
}

impl StrataWeights {
    pub fn validate(&self) -> Result<(), SpecError> {
        let sum = self.critical + self.core + self.hygiene;
        if (sum - 1.0).abs() > 0.001 {
            return Err(SpecError::InvalidWeights(sum));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub target_harness: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteSpec {
    pub suite: SuiteMeta,
    #[serde(default)]
    pub weights: StrataWeights,
    pub cases: Vec<PlumbCaseSpec>,
}

impl SuiteSpec {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, SpecError> {
        let p = path.as_ref();
        let content = fs::read_to_string(p).map_err(|e| SpecError::Io {
            path: p.display().to_string(),
            source: e,
        })?;
        let spec: SuiteSpec = toml::from_str(&content)?;
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), SpecError> {
        self.weights.validate()?;

        for case in &self.cases {
            if case.min_examples < 10 {
                return Err(SpecError::InsufficientMinExamples(
                    case.id.clone(),
                    case.min_examples,
                ));
            }
            if case.alpha <= 0.0 || case.alpha >= 0.5 {
                return Err(SpecError::InvalidAlpha(case.id.clone(), case.alpha));
            }
            if case.max_regression_rate < 0.0 || case.max_regression_rate > 1.0 {
                return Err(SpecError::InvalidRegressionRate(
                    case.id.clone(),
                    case.max_regression_rate,
                ));
            }
        }
        Ok(())
    }

    pub fn find_case(&self, id: &str) -> Result<&PlumbCaseSpec, SpecError> {
        self.cases
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| SpecError::CaseNotFound(id.to_string()))
    }

    pub fn cases_by_stratum(&self) -> BTreeMap<String, Vec<&PlumbCaseSpec>> {
        let mut grouped: BTreeMap<String, Vec<&PlumbCaseSpec>> = BTreeMap::new();
        for case in &self.cases {
            grouped.entry(case.stratum.clone()).or_default().push(case);
        }
        grouped
    }
}
