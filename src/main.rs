#![forbid(unsafe_code)]

use clap::{Args, Parser, Subcommand};
use plumb::{
    audit_repository, calculate_power, evaluate_case, evaluate_suite, export_mermaid_flowchart,
    export_otel_spans, export_reval_ratchet, generate_lockfile,
    ingest::ingest_paired_harness_runs,
    load_and_verify_pairs, render_physics_table,
    simulator::{generate_synthetic_dataset, SimulationProfile},
    verify_lockfile, write_pairs_jsonl, CaseEvaluationResult, HarnessEnvelope, PlumbCaseSpec,
    SuiteEvaluationResult, SuiteSpec,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

const DEFAULT_SOFTFACTORY_TOML: &str = r#"[suite]
name = "softfactory-plumb"
version = "0.1.0"
description = "Soft Factory statistical significance and paired evaluation suite"
target_harness = "agent-harness"

[weights]
critical = 0.50
core = 0.35
hygiene = 0.15

[[cases]]
id = "sf-vsp-rls-paired-win"
stratum = "critical"
severity = "blocker"
description = "VSP Supabase RLS paired migration evaluation: verified superiority with paired 95% CI strictly > 0 and exact McNemar p <= 0.05"
pairs_path = "packs/fixtures/vsp_rls_paired_100.jsonl"
objective = "superiority"
min_examples = 50
alpha = 0.05
min_delta = 0.0
equivalence_margin = 0.05
max_regression_rate = 0.05
bootstrap_iterations = 10000
seed = 42

[[cases]]
id = "sf-orchestrator-surgical-fix"
stratum = "core"
severity = "graded"
description = "TechTideAI2 orchestrator routing fix: surgical win (6 gains, 0 regressions) proven by paired McNemar despite unpaired test missing significance"
pairs_path = "packs/fixtures/orchestrator_surgical_100.jsonl"
objective = "superiority"
min_examples = 50
alpha = 0.05
min_delta = 0.0
equivalence_margin = 0.05
max_regression_rate = 0.05
bootstrap_iterations = 10000
seed = 42

[[cases]]
id = "sf-harness-distillation-tost"
stratum = "hygiene"
severity = "graded"
description = "Agent harness prompt distillation: TOST equivalence test proving model parity within +/-5% at 60% lower token cost"
pairs_path = "packs/fixtures/harness_distillation_100.jsonl"
objective = "equivalence"
min_examples = 50
alpha = 0.05
min_delta = 0.0
equivalence_margin = 0.05
max_regression_rate = 0.05
bootstrap_iterations = 10000
seed = 42
"#;

#[derive(Parser)]
#[command(
    name = "plumb",
    author = "Alex Cinovoj <alex@techtideai.io>",
    version = "0.1.0",
    about = "Paired statistical significance, bootstrapped confidence intervals, exact McNemar tests, and TOST equivalence gating",
    long_about = "Plumb establishes an exact statistical reference datum for TideOS Eval Kit. It verifies whether performance jumps on identical benchmark examples represent genuine capability improvements or statistical noise."
)]
struct Cli {
    #[arg(long, global = true, help = "Emit machine-readable JSON envelope")]
    json: bool,

    #[arg(
        long,
        global = true,
        default_value = "true",
        help = "Ring terminal audio alert bell on failure"
    )]
    bell: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(
        about = "Display reference paired statistical significance & distortion physics table"
    )]
    Table(TableArgs),

    #[command(
        about = "Initialize default softfactory statistical significance suite configuration"
    )]
    Init(InitArgs),

    #[command(about = "Score paired benchmark comparisons against a suite case")]
    Score(ScoreArgs),

    #[command(about = "Run all suite cases and verify gating thresholds")]
    Check(CheckArgs),

    #[command(about = "Compare two evaluation runs or a paired dataset directly")]
    Compare(CompareArgs),

    #[command(about = "Calculate statistical power, sample size requirements, and MDE")]
    Power(PowerArgs),

    #[command(about = "Simulate synthetic paired benchmark datasets with SHA-256 Merkle chain")]
    Simulate(SimulateArgs),

    #[command(about = "Generate sha256-framed-v1 cryptographic lockfile")]
    Lock(LockArgs),

    #[command(about = "Verify suite datasets against cryptographic lockfile")]
    Verify(VerifyArgs),

    #[command(about = "Export suite reports to Mermaid, OpenTelemetry, or reval formats")]
    Export(ExportArgs),

    #[command(about = "Ingest paired runs from agent harness directory")]
    Ingest(IngestArgs),

    #[command(about = "Audit repository for Soft Factory OSS hygiene and zero em-dashes")]
    Repo(RepoArgs),
}

#[derive(Args)]
struct TableArgs {}

#[derive(Args)]
struct InitArgs {
    #[arg(long, default_value = "softfactory")]
    pack: String,
    #[arg(long, default_value = "packs/softfactory.toml")]
    out: PathBuf,
}

#[derive(Args)]
struct ScoreArgs {
    #[arg(long, default_value = "packs/softfactory.toml")]
    suite: PathBuf,
    #[arg(long)]
    case: String,
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

#[derive(Args)]
struct CheckArgs {
    #[arg(long, default_value = "packs/softfactory.toml")]
    suite: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[arg(long)]
    lock: Option<PathBuf>,
}

#[derive(Args)]
struct CompareArgs {
    #[arg(long)]
    pairs: PathBuf,
    #[arg(long, default_value = "10000")]
    bootstrap_iterations: usize,
    #[arg(long, default_value = "42")]
    seed: u64,
    #[arg(long, default_value = "0.05")]
    alpha: f64,
    #[arg(long, default_value = "0.05")]
    equivalence_margin: f64,
}

#[derive(Args)]
struct PowerArgs {
    #[arg(long, default_value = "0.80")]
    baseline_rate: f64,
    #[arg(long, default_value = "0.03")]
    target_delta: f64,
    #[arg(long, default_value = "0.15")]
    discordance_rate: f64,
    #[arg(long, default_value = "0.05")]
    alpha: f64,
    #[arg(long, default_value = "0.80")]
    power: f64,
    #[arg(long, default_value = "100")]
    current_n: usize,
}

#[derive(Args)]
struct SimulateArgs {
    #[arg(long, value_enum, default_value = "high-churn-noise")]
    profile: ProfileChoice,
    #[arg(long, default_value = "100")]
    examples: usize,
    #[arg(long, default_value = "42")]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum ProfileChoice {
    HighChurnNoise,
    Underpowered3Pt,
    SurgicalMonotonicWin,
    RobustBreakthroughWin,
    DistillationEquivalence,
    SignificantRegression,
}

impl From<ProfileChoice> for SimulationProfile {
    fn from(c: ProfileChoice) -> Self {
        match c {
            ProfileChoice::HighChurnNoise => SimulationProfile::HighChurnNoise,
            ProfileChoice::Underpowered3Pt => SimulationProfile::Underpowered3Pt,
            ProfileChoice::SurgicalMonotonicWin => SimulationProfile::SurgicalMonotonicWin,
            ProfileChoice::RobustBreakthroughWin => SimulationProfile::RobustBreakthroughWin,
            ProfileChoice::DistillationEquivalence => SimulationProfile::DistillationEquivalence,
            ProfileChoice::SignificantRegression => SimulationProfile::SignificantRegression,
        }
    }
}

#[derive(Args)]
struct LockArgs {
    #[arg(long, default_value = "packs/softfactory.toml")]
    suite: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[arg(long, default_value = "packs/softfactory.lock.json")]
    out: PathBuf,
}

#[derive(Args)]
struct VerifyArgs {
    #[arg(long, default_value = "packs/softfactory.toml")]
    suite: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[arg(long, default_value = "packs/softfactory.lock.json")]
    lock: PathBuf,
}

#[derive(Args)]
struct ExportArgs {
    #[arg(long, default_value = "packs/softfactory.toml")]
    suite: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[arg(long, value_parser = ["mermaid", "otel", "reval"])]
    format: String,
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct IngestArgs {
    #[arg(long, default_value = "../../Tools/agent-harness/runs")]
    runs_dir: PathBuf,
    #[arg(long, default_value = "claude")]
    baseline: String,
    #[arg(long, default_value = "codex")]
    candidate: String,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct RepoArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
}

fn ring_bell_if(enabled: bool) {
    if enabled {
        eprint!("\x07");
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match execute(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Error: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn execute(cli: Cli) -> anyhow::Result<ExitCode> {
    let json = cli.json;
    let bell = cli.bell;

    match cli.command {
        Commands::Table(_) => {
            let table = render_physics_table();
            if json {
                let env = HarnessEnvelope::success(
                    "Statistical significance and paired evaluation distortion physics table",
                    Vec::new(),
                    table.clone(),
                );
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                print!("{table}");
            }
            Ok(ExitCode::SUCCESS)
        }

        Commands::Init(args) => {
            if let Some(parent) = args.out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&args.out, DEFAULT_SOFTFACTORY_TOML)?;

            if json {
                let env = HarnessEnvelope::success(
                    format!("Scaffolded starter suite to {}", args.out.display()),
                    vec![args.out.display().to_string()],
                    DEFAULT_SOFTFACTORY_TOML,
                );
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                println!(
                    "Initialized {} suite configuration at: {}",
                    args.pack,
                    args.out.display()
                );
            }
            Ok(ExitCode::SUCCESS)
        }

        Commands::Score(args) => {
            let suite = SuiteSpec::load_from_file(&args.suite)?;
            let case = suite.find_case(&args.case)?;
            let result = evaluate_case(case, &args.root);

            if !result.passed {
                ring_bell_if(bell);
            }

            if json {
                let env = if result.passed {
                    HarnessEnvelope::success(
                        format!(
                            "Case '{}' passed (observed delta: {:+.2}%, exact p: {:.4})",
                            result.case_id,
                            result.observed_delta * 100.0,
                            result.raw_p_value
                        ),
                        vec![case.pairs_path.clone()],
                        result.clone(),
                    )
                } else {
                    HarnessEnvelope::error(
                        format!("Case '{}' failed: {:?}", result.case_id, result.regime),
                        result.failure_reasons.clone(),
                        vec![case.pairs_path.clone()],
                        result.clone(),
                    )
                };
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                print_case_report(&result);
            }

            if result.passed {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::from(1))
            }
        }

        Commands::Check(args) => {
            let suite = SuiteSpec::load_from_file(&args.suite)?;

            if let Some(ref lock_path) = args.lock {
                if let Err(err) = verify_lockfile(lock_path, &args.suite, &args.root) {
                    ring_bell_if(bell);
                    if json {
                        let env = HarnessEnvelope::<()>::error(
                            format!("Lockfile verification failed: {err}"),
                            vec!["Run `plumb lock` to refresh cryptographic lockfile".to_string()],
                            vec![lock_path.display().to_string()],
                            (),
                        );
                        println!("{}", serde_json::to_string_pretty(&env)?);
                    } else {
                        eprintln!("Lockfile verification failed: {err}");
                    }
                    return Ok(ExitCode::from(1));
                }
            }

            let suite_result = evaluate_suite(&suite, &args.root);

            if !suite_result.passed {
                ring_bell_if(bell);
            }

            if json {
                let env = if suite_result.passed {
                    HarnessEnvelope::success(
                        format!(
                            "Suite '{}' passed all {} cases (composite score: {:.2}%)",
                            suite_result.suite_name,
                            suite_result.total_cases,
                            suite_result.composite_score * 100.0
                        ),
                        vec![args.suite.display().to_string()],
                        suite_result.clone(),
                    )
                } else {
                    let mut next_actions = Vec::new();
                    for c in &suite_result.cases {
                        if !c.passed {
                            next_actions.push(format!("{}: {:?}", c.case_id, c.regime));
                        }
                    }
                    HarnessEnvelope::error(
                        format!(
                            "Suite '{}' failed: {}/{} passed (blockers failed: {})",
                            suite_result.suite_name,
                            suite_result.passed_cases,
                            suite_result.total_cases,
                            suite_result.blocker_failures
                        ),
                        next_actions,
                        vec![args.suite.display().to_string()],
                        suite_result.clone(),
                    )
                };
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                print_suite_report(&suite_result);
            }

            if suite_result.passed {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::from(1))
            }
        }

        Commands::Compare(args) => {
            let dummy_case = PlumbCaseSpec {
                id: "cli-compare".to_string(),
                stratum: "cli".to_string(),
                severity: plumb::CaseSeverity::Blocker,
                description: "Direct paired comparison via CLI".to_string(),
                pairs_path: args.pairs.display().to_string(),
                objective: plumb::EvaluationObjective::Superiority,
                min_examples: 10,
                alpha: args.alpha,
                min_delta: 0.0,
                equivalence_margin: args.equivalence_margin,
                max_regression_rate: 0.05,
                bootstrap_iterations: args.bootstrap_iterations,
                seed: args.seed,
                require_continuous_significance: false,
            };

            let records = load_and_verify_pairs(&args.pairs)?;
            let result = plumb::evaluator::evaluate_records(&dummy_case, &records);

            if !result.passed {
                ring_bell_if(bell);
            }

            if json {
                let env = HarnessEnvelope::success(
                    format!(
                        "Direct comparison evaluated: observed delta {:+.2}%, exact p = {:.4}",
                        result.observed_delta * 100.0,
                        result.raw_p_value
                    ),
                    vec![args.pairs.display().to_string()],
                    result.clone(),
                );
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                print_case_report(&result);
            }

            if result.passed {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::from(1))
            }
        }

        Commands::Power(args) => {
            let power_calc = calculate_power(
                args.baseline_rate,
                args.target_delta,
                args.discordance_rate,
                args.alpha,
                args.power,
                args.current_n,
            );

            if json {
                let env = HarnessEnvelope::success(
                    format!(
                        "Power analysis: unpaired requires N={}, paired requires N={} ({:.1}% reduction)",
                        power_calc.required_n_unpaired,
                        power_calc.required_n_paired,
                        power_calc.sample_size_reduction_percent
                    ),
                    Vec::new(),
                    power_calc.clone(),
                );
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                println!("\n+-------------------------------------------------------------+");
                println!("|          PLUMB STATISTICAL POWER & SAMPLE SIZE SOLVER       |");
                println!("+-------------------------------------------------------------+");
                println!(
                    "| Baseline Rate (p_A):              {:6.2}%                   |",
                    power_calc.baseline_rate * 100.0
                );
                println!(
                    "| Target Delta (Delta):             {:6.2}%                   |",
                    power_calc.target_delta * 100.0
                );
                println!(
                    "| Discordance Rate (eta):           {:6.2}%                   |",
                    power_calc.discordance_rate * 100.0
                );
                println!(
                    "| Significance Level (alpha):       {:6.4}                    |",
                    power_calc.alpha
                );
                println!(
                    "| Statistical Power (1 - beta):     {:6.2}%                   |",
                    power_calc.power * 100.0
                );
                println!("+-------------------------------------------------------------+");
                println!(
                    "| Required N (Unpaired Two-Arm):    {:6} examples/arm       |",
                    power_calc.required_n_unpaired
                );
                println!(
                    "| Required N (Paired Discordant):   {:6} benchmark examples |",
                    power_calc.required_n_paired
                );
                println!(
                    "| Sample Size Reduction via Pairing:{:6.1}%                   |",
                    power_calc.sample_size_reduction_percent
                );
                println!("+-------------------------------------------------------------+");
                println!("| Current N = {}:", power_calc.current_n);
                println!(
                    "| MDE Paired at current N:          {:6.2}%                   |",
                    power_calc.mde_paired_at_current_n * 100.0
                );
                println!(
                    "| MDE Unpaired at current N:        {:6.2}%                   |",
                    power_calc.mde_unpaired_at_current_n * 100.0
                );
                println!("+-------------------------------------------------------------+\n");
            }
            Ok(ExitCode::SUCCESS)
        }

        Commands::Simulate(args) => {
            let pairs = generate_synthetic_dataset(args.profile.into(), args.examples, args.seed);
            if let Some(parent) = args.out.parent() {
                fs::create_dir_all(parent)?;
            }
            write_pairs_jsonl(&args.out, &pairs)?;

            if json {
                let env = HarnessEnvelope::success(
                    format!(
                        "Generated {} synthetic paired records to {}",
                        pairs.len(),
                        args.out.display()
                    ),
                    vec![args.out.display().to_string()],
                    pairs.len(),
                );
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                println!(
                    "Wrote {} synthetic paired records to {}",
                    pairs.len(),
                    args.out.display()
                );
            }
            Ok(ExitCode::SUCCESS)
        }

        Commands::Lock(args) => {
            let lock = generate_lockfile(&args.suite, &args.root)?;
            if let Some(parent) = args.out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = File::create(&args.out)?;
            let content = serde_json::to_string_pretty(&lock)?;
            file.write_all(content.as_bytes())?;

            if json {
                let env = HarnessEnvelope::success(
                    format!("Generated lockfile at {}", args.out.display()),
                    vec![args.out.display().to_string()],
                    lock,
                );
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                println!(
                    "Generated sha256-framed-v1 lockfile at: {}",
                    args.out.display()
                );
                println!("Overall lock digest: {}", lock.overall_lock_digest);
            }
            Ok(ExitCode::SUCCESS)
        }

        Commands::Verify(args) => {
            verify_lockfile(&args.lock, &args.suite, &args.root)?;
            if json {
                let env = HarnessEnvelope::success(
                    format!(
                        "Lockfile '{}' verified with zero drift",
                        args.lock.display()
                    ),
                    vec![args.lock.display().to_string()],
                    (),
                );
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                println!(
                    "Cryptographic lockfile verified: zero drift detected against {}",
                    args.suite.display()
                );
            }
            Ok(ExitCode::SUCCESS)
        }

        Commands::Export(args) => {
            let suite = SuiteSpec::load_from_file(&args.suite)?;
            let suite_result = evaluate_suite(&suite, &args.root);

            let content = match args.format.as_str() {
                "mermaid" => export_mermaid_flowchart(&suite_result.cases),
                "otel" => serde_json::to_string_pretty(&export_otel_spans(&suite_result, &suite))?,
                "reval" => serde_json::to_string_pretty(&export_reval_ratchet(&suite_result))?,
                _ => anyhow::bail!("Unsupported export format '{}'", args.format),
            };

            if let Some(ref out_path) = args.out {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(out_path, &content)?;
                if json {
                    let env = HarnessEnvelope::success(
                        format!("Exported {} report to {}", args.format, out_path.display()),
                        vec![out_path.display().to_string()],
                        (),
                    );
                    println!("{}", serde_json::to_string_pretty(&env)?);
                } else {
                    println!("Exported {} report to {}", args.format, out_path.display());
                }
            } else {
                println!("{content}");
            }
            Ok(ExitCode::SUCCESS)
        }

        Commands::Ingest(args) => {
            let pairs =
                ingest_paired_harness_runs(&args.runs_dir, &args.baseline, &args.candidate)?;
            if let Some(parent) = args.out.parent() {
                fs::create_dir_all(parent)?;
            }
            write_pairs_jsonl(&args.out, &pairs)?;

            if json {
                let env = HarnessEnvelope::success(
                    format!(
                        "Ingested {} paired runs from {}",
                        pairs.len(),
                        args.runs_dir.display()
                    ),
                    vec![args.out.display().to_string()],
                    pairs.len(),
                );
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                println!(
                    "Ingested {} paired runs into {}",
                    pairs.len(),
                    args.out.display()
                );
            }
            Ok(ExitCode::SUCCESS)
        }

        Commands::Repo(args) => {
            let report = audit_repository(&args.path);
            if !report.passed {
                ring_bell_if(bell);
            }

            if json {
                let env = if report.passed {
                    HarnessEnvelope::success(
                        format!(
                            "Repository audit passed: {}/{} gates verified",
                            report.passed_gates, report.total_gates
                        ),
                        vec![args.path.display().to_string()],
                        report.clone(),
                    )
                } else {
                    let failures: Vec<String> = report
                        .gates
                        .iter()
                        .filter(|g| !g.passed)
                        .map(|g| format!("{}: {}", g.name, g.details))
                        .collect();
                    HarnessEnvelope::error(
                        format!(
                            "Repository audit failed: {}/{} gates passed",
                            report.passed_gates, report.total_gates
                        ),
                        failures,
                        vec![args.path.display().to_string()],
                        report.clone(),
                    )
                };
                println!("{}", serde_json::to_string_pretty(&env)?);
            } else {
                println!("\n+-------------------------------------------------------------+");
                println!("|         SOFT FACTORY REPOSITORY HYGIENE & TYPOGRAPHY GATE    |");
                println!("+-------------------------------------------------------------+");
                for g in &report.gates {
                    let status = if g.passed { "PASS" } else { "FAIL" };
                    println!("| [{}] {:<42} |", status, g.name);
                    if !g.passed {
                        println!("|       -> {:<50} |", g.details);
                    }
                }
                println!("+-------------------------------------------------------------+");
                println!(
                    "| Result: {}/{} Gates Passed                                    |",
                    report.passed_gates, report.total_gates
                );
                println!("+-------------------------------------------------------------+\n");
            }

            if report.passed {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::from(1))
            }
        }
    }
}

fn print_case_report(res: &CaseEvaluationResult) {
    println!("\n+-----------------------------------------------------------------------------+");
    println!("|                     PLUMB STATISTICAL EVALUATION REPORT                     |");
    println!("+-----------------------------------------------------------------------------+");
    println!("| Case ID:                   {:<48} |", res.case_id);
    println!(
        "| Stratum / Severity:        {:<48} |",
        format!("{} ({:?})", res.stratum, res.severity)
    );
    println!(
        "| Status / Regime:           {:<48} |",
        format!(
            "{} [{:?}]",
            if res.passed { "PASS" } else { "FAIL" },
            res.regime
        )
    );
    println!("+-----------------------------------------------------------------------------+");
    println!("| Total Paired Examples:     {:<48} |", res.total_examples);
    println!(
        "| Baseline Rate:             {:<48} |",
        format!(
            "{:.2}% (Wald +/-{:.2}%, Wilson [{:.2}%, {:.2}%])",
            res.baseline_rate * 100.0,
            res.baseline_wald_ci.margin * 100.0,
            res.baseline_wilson_ci.lower * 100.0,
            res.baseline_wilson_ci.upper * 100.0
        )
    );
    println!(
        "| Candidate Rate:            {:<48} |",
        format!(
            "{:.2}% (Wald +/-{:.2}%, Wilson [{:.2}%, {:.2}%])",
            res.candidate_rate * 100.0,
            res.candidate_wald_ci.margin * 100.0,
            res.candidate_wilson_ci.lower * 100.0,
            res.candidate_wilson_ci.upper * 100.0
        )
    );
    println!(
        "| Observed Delta:            {:<48} |",
        format!("{:+.2}%", res.observed_delta * 100.0)
    );
    println!(
        "| Regression Rate:           {:<48} |",
        format!(
            "{:.2}% ({} broken)",
            res.regression_rate * 100.0,
            res.discordance.n_regression
        )
    );
    println!("+-----------------------------------------------------------------------------+");
    println!("| 2x2 Discordance Matrix:                                                     |");
    println!(
        "|   Both Pass (n11): {:<5} | Gains (n01):       {:<5}                         |",
        res.discordance.n_both_pass, res.discordance.n_gain
    );
    println!(
        "|   Both Fail (n00): {:<5} | Regressions (n10): {:<5}                         |",
        res.discordance.n_both_fail, res.discordance.n_regression
    );
    println!(
        "|   Discordant (nd): {:<5} | Variance Reduction:{:<5.2}x                      |",
        res.discordance.discordant_count, res.discordance.variance_reduction_factor
    );
    println!("+-----------------------------------------------------------------------------+");
    println!("| Hypothesis Testing:                                                         |");
    println!(
        "|   Exact McNemar Two-Sided p: {:<46} |",
        format!("{:.6}", res.mcnemar.p_value_exact)
    );
    println!(
        "|   McNemar Mid-p:             {:<46} |",
        format!("{:.6}", res.mcnemar.p_value_mid)
    );
    println!(
        "|   Permutation Test p:        {:<46} |",
        format!("{:.6}", res.permutation_p_value)
    );
    println!(
        "|   Paired Bootstrap 95% CI:   {:<46} |",
        format!(
            "[{:+.2}%, {:+.2}%]",
            res.bootstrap.percentile_95_ci.lower * 100.0,
            res.bootstrap.percentile_95_ci.upper * 100.0
        )
    );
    println!(
        "|   BCa Accelerated 95% CI:    {:<46} |",
        format!(
            "[{:+.2}%, {:+.2}%]",
            res.bootstrap.bca_95_ci.lower * 100.0,
            res.bootstrap.bca_95_ci.upper * 100.0
        )
    );
    if let Some(ref t) = res.tost {
        println!(
            "|   TOST Equivalence p:        {:<46} |",
            format!(
                "{:.6} (margin +/-{:.1}%)",
                t.p_value_tost,
                t.equivalence_margin * 100.0
            )
        );
    }
    if !res.failure_reasons.is_empty() {
        println!("+-----------------------------------------------------------------------------+");
        println!("| Failure Findings:                                                           |");
        for f in &res.failure_reasons {
            println!("|   * {:<71} |", f);
        }
    }
    println!("+-----------------------------------------------------------------------------+\n");
}

fn print_suite_report(res: &SuiteEvaluationResult) {
    println!("\n+-----------------------------------------------------------------------------+");
    println!("|                  TIDEOS EVAL KIT - SUITE EVALUATION REPORT                  |");
    println!("+-----------------------------------------------------------------------------+");
    println!(
        "| Suite:                     {:<48} |",
        format!("{} v{}", res.suite_name, res.suite_version)
    );
    println!(
        "| Status:                    {:<48} |",
        if res.passed {
            "PASS (ALL GATES CLEARED)"
        } else {
            "FAIL (GATES TRIPPED)"
        }
    );
    println!(
        "| Composite Quality Score:   {:<48} |",
        format!("{:.2}%", res.composite_score * 100.0)
    );
    println!(
        "| Total Cases:               {:<48} |",
        format!("{}/{} passed", res.passed_cases, res.total_cases)
    );
    println!("+-----------------------------------------------------------------------------+");
    for (s_name, s_data) in &res.strata {
        println!(
            "| Stratum {:<10} (w={:.2}): {:>2}/{} passed ({:5.1}%)                          |",
            s_name,
            s_data.weight,
            s_data.passed_cases,
            s_data.total_cases,
            s_data.pass_rate * 100.0
        );
    }
    println!("+-----------------------------------------------------------------------------+");
    for c in &res.cases {
        let flag = if c.passed { "PASS" } else { "FAIL" };
        println!(
            "| [{}] {:<30} Delta: {:>+6.2}% | p={:<8.4} | {:<12} |",
            flag,
            c.case_id,
            c.observed_delta * 100.0,
            c.raw_p_value,
            format!("{:?}", c.regime)
        );
    }
    println!("+-----------------------------------------------------------------------------+\n");
}
