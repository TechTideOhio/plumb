#![forbid(unsafe_code)]

pub mod economics;
pub mod envelope;
pub mod evaluator;
pub mod ingest;
pub mod lockfile;
pub mod pair;
pub mod repo_audit;
pub mod report;
pub mod simulator;
pub mod spec;
pub mod stats;

pub use envelope::{EnvelopeStatus, HarnessEnvelope};
pub use evaluator::{
    evaluate_case, evaluate_records, evaluate_suite, CaseEvaluationResult, PlumbRegime,
    SuiteEvaluationResult,
};
pub use lockfile::{generate_lockfile, verify_lockfile, PlumbLockfile};
pub use pair::{load_and_verify_pairs, write_pairs_jsonl, PairedRecord, GENESIS_HASH};
pub use repo_audit::{audit_repository, RepoAuditReport};
pub use report::{
    export_mermaid_flowchart, export_otel_spans, export_reval_ratchet, render_physics_table,
};
pub use spec::{CaseSeverity, EvaluationObjective, PlumbCaseSpec, StrataWeights, SuiteSpec};
pub use stats::{
    calculate_power, exact_mcnemar_test, paired_bootstrap_ci, paired_sign_flip_permutation_test,
    tost_equivalence_test, wald_interval, wilcoxon_signed_rank_test, wilson_score_interval,
    BootstrapResult, ConfidenceInterval, DiscordanceMatrix, McNemarResult, PowerCalculation,
    SplitMix64, TostResult, WilcoxonResult,
};
