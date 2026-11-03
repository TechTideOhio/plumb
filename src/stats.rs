use serde::{Deserialize, Serialize};

pub const DEFAULT_BOOTSTRAP_ITERATIONS: usize = 10_000;
pub const DEFAULT_SEED: u64 = 42;
pub const Z_ALPHA_05: f64 = 1.959_963_984_540_054;
pub const Z_POWER_80: f64 = 0.841_621_233_572_914_3;

#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
    }

    pub fn gen_range(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        (self.next_u64() % (upper as u64)) as usize
    }

    pub fn next_bool(&mut self) -> bool {
        (self.next_u64() & 1) == 1
    }
}

pub fn ln_gamma(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let p = [
        676.520_368_121_885_1,
        -1259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_822_208_94,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let g = 7.0;
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        (pi / ((pi * x).sin() * (ln_gamma(1.0 - x)).exp())).ln()
    } else {
        let z = x - 1.0;
        let mut sum = 0.999_999_999_999_809_9;
        for (i, &pval) in p.iter().enumerate() {
            sum += pval / (z + (i as f64) + 1.0);
        }
        let t = z + g + 0.5;
        0.5 * (2.0 * std::f64::consts::PI).ln() + (z + 0.5) * t.ln() - t + sum.ln()
    }
}

pub fn ln_combination(n: usize, k: usize) -> f64 {
    if k > n {
        return f64::NEG_INFINITY;
    }
    if k == 0 || k == n {
        return 0.0;
    }
    ln_gamma((n + 1) as f64) - ln_gamma((k + 1) as f64) - ln_gamma((n - k + 1) as f64)
}

pub fn normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

pub fn normal_inv_cdf(p: f64) -> f64 {
    if p <= 0.0 {
        return -8.0;
    }
    if p >= 1.0 {
        return 8.0;
    }
    if (p - 0.5).abs() < 1e-12 {
        return 0.0;
    }

    let a = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_690e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    let b = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    let c = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    let d = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];

    let p_low = 0.02425;
    let p_high = 1.0 - p_low;

    if p < p_low {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= p_high {
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
            / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    }
}

fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.5 * x.abs());
    let tau = t
        * (-x * x - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87
                                        + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x >= 0.0 {
        1.0 - tau
    } else {
        tau - 1.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfidenceInterval {
    pub point: f64,
    pub lower: f64,
    pub upper: f64,
    pub margin: f64,
    pub confidence_level: f64,
}

impl ConfidenceInterval {
    pub fn contains(&self, val: f64) -> bool {
        val >= self.lower && val <= self.upper
    }

    pub fn strictly_greater_than(&self, val: f64) -> bool {
        self.lower > val
    }

    pub fn strictly_less_than(&self, val: f64) -> bool {
        self.upper < val
    }
}

pub fn wald_interval(successes: usize, total: usize, confidence_level: f64) -> ConfidenceInterval {
    if total == 0 {
        return ConfidenceInterval {
            point: 0.0,
            lower: 0.0,
            upper: 0.0,
            margin: 0.0,
            confidence_level,
        };
    }
    let p = (successes as f64) / (total as f64);
    let alpha = 1.0 - confidence_level;
    let z = normal_inv_cdf(1.0 - alpha / 2.0);
    let se = (p * (1.0 - p) / (total as f64)).sqrt();
    let margin = z * se;

    ConfidenceInterval {
        point: p,
        lower: (p - margin).max(0.0),
        upper: (p + margin).min(1.0),
        margin,
        confidence_level,
    }
}

pub fn wilson_score_interval(
    successes: usize,
    total: usize,
    confidence_level: f64,
) -> ConfidenceInterval {
    if total == 0 {
        return ConfidenceInterval {
            point: 0.0,
            lower: 0.0,
            upper: 0.0,
            margin: 0.0,
            confidence_level,
        };
    }
    let p = (successes as f64) / (total as f64);
    let n = total as f64;
    let alpha = 1.0 - confidence_level;
    let z = normal_inv_cdf(1.0 - alpha / 2.0);
    let z2 = z * z;

    let denom = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denom;
    let half_width = (z / denom) * ((p * (1.0 - p) / n) + (z2 / (4.0 * n * n))).sqrt();

    ConfidenceInterval {
        point: p,
        lower: (center - half_width).max(0.0),
        upper: (center + half_width).min(1.0),
        margin: half_width,
        confidence_level,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscordanceMatrix {
    pub n_both_pass: usize,
    pub n_both_fail: usize,
    pub n_gain: usize,
    pub n_regression: usize,
    pub total: usize,
    pub discordant_count: usize,
    pub discordance_rate: f64,
    pub baseline_rate: f64,
    pub candidate_rate: f64,
    pub delta: f64,
    pub phi_correlation: f64,
    pub se_unpaired: f64,
    pub se_paired: f64,
    pub variance_reduction_factor: f64,
}

impl DiscordanceMatrix {
    pub fn from_counts(
        n_both_pass: usize,
        n_both_fail: usize,
        n_gain: usize,
        n_regression: usize,
    ) -> Self {
        let total = n_both_pass + n_both_fail + n_gain + n_regression;
        let discordant_count = n_gain + n_regression;
        let n_f64 = total as f64;

        let baseline_pass = n_both_pass + n_regression;
        let candidate_pass = n_both_pass + n_gain;

        let p_a = if total > 0 {
            (baseline_pass as f64) / n_f64
        } else {
            0.0
        };
        let p_b = if total > 0 {
            (candidate_pass as f64) / n_f64
        } else {
            0.0
        };
        let delta = p_b - p_a;
        let discordance_rate = if total > 0 {
            (discordant_count as f64) / n_f64
        } else {
            0.0
        };

        let se_unpaired = if total > 0 {
            ((p_a * (1.0 - p_a) + p_b * (1.0 - p_b)) / n_f64).sqrt()
        } else {
            0.0
        };

        let se_paired = if total > 0 {
            let num =
                (discordant_count as f64) - ((n_gain as f64 - n_regression as f64).powi(2) / n_f64);
            (num.max(0.0).sqrt()) / n_f64
        } else {
            0.0
        };

        let num_phi =
            (n_both_pass as f64) * (n_both_fail as f64) - (n_regression as f64) * (n_gain as f64);
        let denom_phi = ((n_both_pass + n_regression) as f64
            * (n_gain + n_both_fail) as f64
            * (n_both_pass + n_gain) as f64
            * (n_regression + n_both_fail) as f64)
            .sqrt();

        let phi_correlation = if denom_phi > 0.0 {
            num_phi / denom_phi
        } else {
            0.0
        };

        let var_unpaired = se_unpaired * se_unpaired;
        let var_paired = se_paired * se_paired;
        let variance_reduction_factor = if var_paired > 1e-12 {
            var_unpaired / var_paired
        } else if var_unpaired > 1e-12 {
            10.0
        } else {
            1.0
        };

        Self {
            n_both_pass,
            n_both_fail,
            n_gain,
            n_regression,
            total,
            discordant_count,
            discordance_rate,
            baseline_rate: p_a,
            candidate_rate: p_b,
            delta,
            phi_correlation,
            se_unpaired,
            se_paired,
            variance_reduction_factor,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McNemarResult {
    pub p_value_exact: f64,
    pub p_value_mid: f64,
    pub p_value_asymptotic: f64,
    pub discordant_count: usize,
    pub n_gain: usize,
    pub n_regression: usize,
    pub statistic_chi2: f64,
}

pub fn exact_mcnemar_test(n_gain: usize, n_regression: usize) -> McNemarResult {
    let n_d = n_gain + n_regression;
    if n_d == 0 {
        return McNemarResult {
            p_value_exact: 1.0,
            p_value_mid: 1.0,
            p_value_asymptotic: 1.0,
            discordant_count: 0,
            n_gain,
            n_regression,
            statistic_chi2: 0.0,
        };
    }

    let min_k = n_gain.min(n_regression);
    let mut cumulative_prob = 0.0;
    let mut boundary_prob = 0.0;

    for k in 0..=min_k {
        let ln_prob = ln_combination(n_d, k) - (n_d as f64) * 2.0_f64.ln();
        let prob = ln_prob.exp();
        cumulative_prob += prob;
        if k == min_k {
            boundary_prob = prob;
        }
    }

    let p_two_sided = (2.0 * cumulative_prob).min(1.0);
    let p_mid = (p_two_sided - boundary_prob).max(0.0).min(1.0);

    let diff = (n_gain as f64) - (n_regression as f64);
    let chi2 = (diff.abs() - 1.0).max(0.0).powi(2) / (n_d as f64);
    let p_asymptotic = 2.0 * (1.0 - normal_cdf(chi2.sqrt()));

    McNemarResult {
        p_value_exact: p_two_sided,
        p_value_mid: p_mid,
        p_value_asymptotic: p_asymptotic,
        discordant_count: n_d,
        n_gain,
        n_regression,
        statistic_chi2: chi2,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BootstrapResult {
    pub delta_mean: f64,
    pub delta_median: f64,
    pub se_bootstrap: f64,
    pub percentile_95_ci: ConfidenceInterval,
    pub bca_95_ci: ConfidenceInterval,
    pub iterations: usize,
    pub seed: u64,
}

pub fn paired_bootstrap_ci(
    deltas: &[f64],
    iterations: usize,
    seed: u64,
    confidence_level: f64,
) -> BootstrapResult {
    let n = deltas.len();
    if n == 0 {
        let zero_ci = ConfidenceInterval {
            point: 0.0,
            lower: 0.0,
            upper: 0.0,
            margin: 0.0,
            confidence_level,
        };
        return BootstrapResult {
            delta_mean: 0.0,
            delta_median: 0.0,
            se_bootstrap: 0.0,
            percentile_95_ci: zero_ci.clone(),
            bca_95_ci: zero_ci,
            iterations,
            seed,
        };
    }

    let orig_mean = deltas.iter().sum::<f64>() / (n as f64);
    let mut rng = SplitMix64::new(seed);
    let mut boot_means = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let mut sum = 0.0;
        for _ in 0..n {
            let idx = rng.gen_range(n);
            sum += deltas[idx];
        }
        boot_means.push(sum / (n as f64));
    }

    boot_means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let sum_boot: f64 = boot_means.iter().sum();
    let mean_boot = sum_boot / (iterations as f64);
    let variance_boot: f64 = boot_means
        .iter()
        .map(|&m| (m - mean_boot).powi(2))
        .sum::<f64>()
        / (iterations as f64);
    let se_boot = variance_boot.sqrt();

    let alpha = 1.0 - confidence_level;
    let idx_low = ((alpha / 2.0) * (iterations as f64)).floor() as usize;
    let idx_high =
        (((1.0 - alpha / 2.0) * (iterations as f64)).ceil() as usize).min(iterations - 1);

    let perc_lower = boot_means[idx_low];
    let perc_upper = boot_means[idx_high];
    let median_boot = boot_means[iterations / 2];

    let perc_ci = ConfidenceInterval {
        point: orig_mean,
        lower: perc_lower,
        upper: perc_upper,
        margin: (perc_upper - perc_lower) / 2.0,
        confidence_level,
    };

    // BCa calculation
    let count_less = boot_means.iter().filter(|&&m| m < orig_mean).count();
    let p_less = ((count_less as f64) + 0.5) / (iterations as f64);
    let z0 = normal_inv_cdf(p_less.clamp(1e-6, 1.0 - 1e-6));

    let total_sum: f64 = deltas.iter().sum();
    let mut jackknife_means = Vec::with_capacity(n);
    if n > 1 {
        for &d in deltas {
            jackknife_means.push((total_sum - d) / ((n - 1) as f64));
        }
    } else {
        jackknife_means.push(orig_mean);
    }

    let jack_mean = jackknife_means.iter().sum::<f64>() / (n as f64);
    let mut num_acc = 0.0;
    let mut denom_acc = 0.0;

    for &jm in &jackknife_means {
        let diff = jack_mean - jm;
        num_acc += diff.powi(3);
        denom_acc += diff.powi(2);
    }

    let a = if denom_acc > 1e-12 {
        num_acc / (6.0 * denom_acc.powf(1.5))
    } else {
        0.0
    };

    let z_alpha_low = normal_inv_cdf(alpha / 2.0);
    let z_alpha_high = normal_inv_cdf(1.0 - alpha / 2.0);

    let calc_bca_p = |z_alpha: f64| -> f64 {
        let denom = 1.0 - a * (z0 + z_alpha);
        if denom.abs() < 1e-6 {
            normal_cdf(z0 + z_alpha)
        } else {
            let arg = z0 + (z0 + z_alpha) / denom;
            normal_cdf(arg)
        }
    };

    let p_bca_low = calc_bca_p(z_alpha_low).clamp(0.0001, 0.9999);
    let p_bca_high = calc_bca_p(z_alpha_high).clamp(0.0001, 0.9999);

    let idx_bca_low = ((p_bca_low * (iterations as f64)).floor() as usize).min(iterations - 1);
    let idx_bca_high =
        (((p_bca_high * (iterations as f64)).ceil() as usize).min(iterations - 1)).max(idx_bca_low);

    let bca_ci = ConfidenceInterval {
        point: orig_mean,
        lower: boot_means[idx_bca_low],
        upper: boot_means[idx_bca_high],
        margin: (boot_means[idx_bca_high] - boot_means[idx_bca_low]) / 2.0,
        confidence_level,
    };

    BootstrapResult {
        delta_mean: mean_boot,
        delta_median: median_boot,
        se_bootstrap: se_boot,
        percentile_95_ci: perc_ci,
        bca_95_ci: bca_ci,
        iterations,
        seed,
    }
}

pub fn paired_sign_flip_permutation_test(deltas: &[f64], iterations: usize, seed: u64) -> f64 {
    let n = deltas.len();
    if n == 0 {
        return 1.0;
    }
    let non_zero: Vec<f64> = deltas
        .iter()
        .copied()
        .filter(|&d| d.abs() > 1e-12)
        .collect();
    if non_zero.is_empty() {
        return 1.0;
    }

    let obs_stat: f64 = (non_zero.iter().sum::<f64>() / (n as f64)).abs();
    let mut rng = SplitMix64::new(seed);
    let mut count_extreme = 0;

    for _ in 0..iterations {
        let mut perm_sum = 0.0;
        for &d in &non_zero {
            let sign = if rng.next_bool() { 1.0 } else { -1.0 };
            perm_sum += sign * d;
        }
        let perm_stat = (perm_sum / (n as f64)).abs();
        if perm_stat >= obs_stat - 1e-12 {
            count_extreme += 1;
        }
    }

    (count_extreme as f64) / (iterations as f64)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WilcoxonResult {
    pub statistic_w_pos: f64,
    pub statistic_w_neg: f64,
    pub z_score: f64,
    pub p_value: f64,
    pub non_zero_count: usize,
}

pub fn wilcoxon_signed_rank_test(deltas: &[f64]) -> WilcoxonResult {
    let mut pairs: Vec<f64> = deltas
        .iter()
        .copied()
        .filter(|&d| d.abs() > 1e-12)
        .collect();
    let nr = pairs.len();
    if nr == 0 {
        return WilcoxonResult {
            statistic_w_pos: 0.0,
            statistic_w_neg: 0.0,
            z_score: 0.0,
            p_value: 1.0,
            non_zero_count: 0,
        };
    }

    pairs.sort_by(|a, b| {
        a.abs()
            .partial_cmp(&b.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut ranks = vec![0.0; nr];
    let mut i = 0;
    while i < nr {
        let mut j = i;
        while j < nr && (pairs[j].abs() - pairs[i].abs()).abs() < 1e-12 {
            j += 1;
        }
        let avg_rank = ((i + 1 + j) as f64) / 2.0;
        for r in ranks.iter_mut().take(j).skip(i) {
            *r = avg_rank;
        }
        i = j;
    }

    let mut w_pos = 0.0;
    let mut w_neg = 0.0;
    for (idx, &val) in pairs.iter().enumerate() {
        if val > 0.0 {
            w_pos += ranks[idx];
        } else {
            w_neg += ranks[idx];
        }
    }

    let nr_f = nr as f64;
    let mean_w = nr_f * (nr_f + 1.0) / 4.0;
    let var_w = nr_f * (nr_f + 1.0) * (2.0 * nr_f + 1.0) / 24.0;
    let se_w = var_w.sqrt();

    let z = if se_w > 0.0 {
        let diff = w_pos - mean_w;
        let cont_corr = if diff > 0.0 {
            -0.5
        } else if diff < 0.0 {
            0.5
        } else {
            0.0
        };
        (diff + cont_corr) / se_w
    } else {
        0.0
    };

    let p_value = (2.0 * (1.0 - normal_cdf(z.abs()))).min(1.0);

    WilcoxonResult {
        statistic_w_pos: w_pos,
        statistic_w_neg: w_neg,
        z_score: z,
        p_value,
        non_zero_count: nr,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TostResult {
    pub is_equivalent: bool,
    pub equivalence_margin: f64,
    pub p_value_tost: f64,
    pub z_lower: f64,
    pub z_upper: f64,
    pub p_lower: f64,
    pub p_upper: f64,
}

pub fn tost_equivalence_test(
    delta: f64,
    se_paired: f64,
    equivalence_margin: f64,
    alpha: f64,
) -> TostResult {
    let margin = equivalence_margin.abs();
    if se_paired <= 1e-12 {
        let eq = delta.abs() <= margin;
        return TostResult {
            is_equivalent: eq,
            equivalence_margin: margin,
            p_value_tost: if eq { 0.0 } else { 1.0 },
            z_lower: 0.0,
            z_upper: 0.0,
            p_lower: if eq { 0.0 } else { 1.0 },
            p_upper: if eq { 0.0 } else { 1.0 },
        };
    }

    // H01: delta <= -margin -> z1 = (delta - (-margin)) / SE
    let z1 = (delta + margin) / se_paired;
    let p1 = 1.0 - normal_cdf(z1);

    // H02: delta >= +margin -> z2 = (+margin - delta) / SE
    let z2 = (margin - delta) / se_paired;
    let p2 = 1.0 - normal_cdf(z2);

    let p_tost = p1.max(p2);
    let is_equivalent = p_tost <= alpha;

    TostResult {
        is_equivalent,
        equivalence_margin: margin,
        p_value_tost: p_tost,
        z_lower: z1,
        z_upper: z2,
        p_lower: p1,
        p_upper: p2,
    }
}

pub fn benjamini_hochberg_adjust(p_values: &[f64]) -> Vec<f64> {
    let m = p_values.len();
    if m == 0 {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, f64)> = p_values.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut adjusted = vec![0.0; m];
    let mut min_q = 1.0;

    for i in (0..m).rev() {
        let orig_idx = indexed[i].0;
        let p_val = indexed[i].1;
        let rank = (i + 1) as f64;
        let q = (p_val * (m as f64) / rank).min(1.0);
        if q < min_q {
            min_q = q;
        }
        adjusted[orig_idx] = min_q;
    }

    adjusted
}

pub fn holm_bonferroni_adjust(p_values: &[f64]) -> Vec<f64> {
    let m = p_values.len();
    if m == 0 {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, f64)> = p_values.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut adjusted = vec![0.0; m];
    let mut max_p = 0.0;

    for (rank_0, item) in indexed.iter().enumerate() {
        let orig_idx = item.0;
        let p_val = item.1;
        let multiplier = (m - rank_0) as f64;
        let p_adj = (p_val * multiplier).min(1.0);
        if p_adj > max_p {
            max_p = p_adj;
        }
        adjusted[orig_idx] = max_p;
    }

    adjusted
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PowerCalculation {
    pub baseline_rate: f64,
    pub target_delta: f64,
    pub discordance_rate: f64,
    pub alpha: f64,
    pub power: f64,
    pub required_n_unpaired: usize,
    pub required_n_paired: usize,
    pub sample_size_reduction_percent: f64,
    pub current_n: usize,
    pub mde_paired_at_current_n: f64,
    pub mde_unpaired_at_current_n: f64,
}

pub fn calculate_power(
    baseline_rate: f64,
    target_delta: f64,
    discordance_rate: f64,
    alpha: f64,
    power: f64,
    current_n: usize,
) -> PowerCalculation {
    let z_alpha = normal_inv_cdf(1.0 - alpha / 2.0);
    let z_beta = normal_inv_cdf(power);

    let p_a = baseline_rate.clamp(0.01, 0.99);
    let p_b = (p_a + target_delta).clamp(0.01, 0.99);
    let delta = (p_b - p_a).abs().max(0.001);

    // Unpaired required N
    let p_bar = (p_a + p_b) / 2.0;
    let num_unp = z_alpha * (2.0 * p_bar * (1.0 - p_bar)).sqrt()
        + z_beta * (p_a * (1.0 - p_a) + p_b * (1.0 - p_b)).sqrt();
    let req_n_unp = ((num_unp / delta).powi(2)).ceil() as usize;

    // Paired required N based on discordance rate eta
    let eta = discordance_rate.clamp(delta, 0.99);
    let num_pair = (z_alpha + z_beta).powi(2) * eta * (1.0 - (delta * delta) / eta);
    let req_n_pair = ((num_pair / (delta * delta)).ceil() as usize).max(10);

    let red_pct = if req_n_unp > 0 {
        100.0 * (1.0 - (req_n_pair as f64) / (req_n_unp as f64))
    } else {
        0.0
    };

    let n_f = (current_n.max(10)) as f64;
    let mde_paired = (z_alpha + z_beta) * (eta.sqrt()) / n_f.sqrt();
    let mde_unpaired =
        (z_alpha + z_beta) * (p_a * (1.0 - p_a) + p_b * (1.0 - p_b)).sqrt() / n_f.sqrt();

    PowerCalculation {
        baseline_rate: p_a,
        target_delta: delta,
        discordance_rate: eta,
        alpha,
        power,
        required_n_unpaired: req_n_unp,
        required_n_paired: req_n_pair,
        sample_size_reduction_percent: red_pct.max(0.0),
        current_n,
        mde_paired_at_current_n: mde_paired,
        mde_unpaired_at_current_n: mde_unpaired,
    }
}
