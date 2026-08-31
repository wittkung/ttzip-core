// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Layer 3 Statistical Analysis & Measurement Kernel for Declarative A/B Benchmarking.
//!
//! Features:
//! - Clock rising-edge alignment (`sync_to_next_tick`) to eliminate sub-tick jitter.
//! - Hampel 3-sigma MAD (Median Absolute Deviation) robust outlier filter.
//! - Lanczos log-gamma and continued-fraction regularized incomplete beta functions.
//! - Welch's heteroscedastic two-sample t-test with Welch-Satterthwaite degrees of freedom.
//! - Exact two-tailed p-value and 95% two-sided confidence intervals.
//! - Adaptive measurement engine with warmup and RSE <= 0.5% early-stopping convergence.

use std::f64::consts::PI;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::benchmark::ab_engine::timing::wait_for_next_tick_instant;

/// Synchronizes execution with the rising edge of the monotonic clock.
#[inline]
pub fn sync_to_next_tick() -> Instant {
    wait_for_next_tick_instant()
}

// ============================================================================
// Special Mathematical Functions (Lanczos Gamma, Incomplete Beta, Student's t)
// ============================================================================

/// Computes the natural logarithm of the Gamma function $\ln\Gamma(z)$ using the Lanczos approximation.
pub fn lanczos_lgamma(z: f64) -> f64 {
    if z < 0.5 {
        let sin_pi_z = (PI * z).sin();
        if sin_pi_z.abs() < 1e-15 {
            return f64::INFINITY;
        }
        PI.ln() - sin_pi_z.abs().ln() - lanczos_lgamma(1.0 - z)
    } else {
        const COEFFS: [f64; 6] = [
            76.18009172947146,
            -86.50532032941677,
            24.01409824083091,
            -1.231739572450155,
            0.1208650973866179e-2,
            -0.5395239384953e-5,
        ];
        let x = z;
        let mut y = z;
        let tmp = x + 5.5;
        let term = (x + 0.5) * tmp.ln() - tmp;
        let mut ser = 1.000000000190015;
        for &c in &COEFFS {
            y += 1.0;
            ser += c / y;
        }
        term + (2.5066282746310005 * ser / x).ln()
    }
}

/// Computes the continued fraction component for the regularized incomplete beta function (Lentz's method).
fn betacf(a: f64, b: f64, x: f64) -> f64 {
    const MAX_IT: usize = 200;
    const EPS: f64 = 1e-15;
    const FPMIN: f64 = 1e-30;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = (1.0 - qab * x / qap).max(FPMIN);
    d = 1.0 / d;
    let mut h = d;

    for m in 1..=MAX_IT {
        let m_f = m as f64;
        let m2 = 2.0 * m_f;

        // Even step (2m)
        let aa = m_f * (b - m_f) * x / ((qam + m2) * (a + m2));
        d = (1.0 + aa * d).max(FPMIN);
        c = (1.0 + aa / c).max(FPMIN);
        d = 1.0 / d;
        h *= d * c;

        // Odd step (2m + 1)
        let aa = -(a + m_f) * (qab + m_f) * x / ((a + m2) * (qap + m2));
        d = (1.0 + aa * d).max(FPMIN);
        c = (1.0 + aa / c).max(FPMIN);
        d = 1.0 / d;
        let del = d * c;
        h *= del;

        if (del - 1.0).abs() < EPS {
            break;
        }
    }
    h
}

/// Computes the regularized incomplete beta function $I_x(a, b)$.
pub fn inc_beta_reg(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    if a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }

    let ln_beta = lanczos_lgamma(a) + lanczos_lgamma(b) - lanczos_lgamma(a + b);
    let bt = (a * x.ln() + b * (1.0 - x).ln() - ln_beta).exp();

    if x < (a + 1.0) / (a + b + 2.0) {
        (bt * betacf(a, b, x) / a).clamp(0.0, 1.0)
    } else {
        (1.0 - bt * betacf(b, a, 1.0 - x) / b).clamp(0.0, 1.0)
    }
}

/// Standard normal inverse cumulative distribution function (Acklam's rational approximation).
pub fn inverse_normal_cdf(p: f64) -> f64 {
    let p_clamped = p.clamp(1e-15, 1.0 - 1e-15);
    const A: [f64; 6] = [-39.69683028665376, 220.9460984245205, -275.9285104469687, 138.357_751_867_269, -30.66479806614716, 2.506628277459239];
    const B: [f64; 5] = [-54.47609879822406, 161.5858368580409, -155.6989798598866, 66.80131188771972, -13.28068155288572];
    const C: [f64; 6] = [-0.007784894002430293, -0.3223964580411365, -2.400758277161838, -2.549732539343734, 4.374664141464968, 2.938163982698783];
    const D: [f64; 4] = [0.007784695709041462, 0.3224671290700398, 2.445134137142996, 3.754408661907416];

    let p_low = 0.02425;
    let p_high = 1.0 - p_low;

    if p_clamped < p_low {
        let q = (-2.0 * p_clamped.ln()).sqrt();
        (((((C[0]*q + C[1])*q + C[2])*q + C[3])*q + C[4])*q + C[5]) / ((((D[0]*q + D[1])*q + D[2])*q + D[3])*q + 1.0)
    } else if p_clamped <= p_high {
        let q = p_clamped - 0.5;
        let r = q * q;
        (((((A[0]*r + A[1])*r + A[2])*r + A[3])*r + A[4])*r + A[5]) * q / (((((B[0]*r + B[1])*r + B[2])*r + B[3])*r + B[4])*r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p_clamped).ln()).sqrt();
        -(((((C[0]*q + C[1])*q + C[2])*q + C[3])*q + C[4])*q + C[5]) / ((((D[0]*q + D[1])*q + D[2])*q + D[3])*q + 1.0)
    }
}

/// Computes the exact two-tailed p-value for Student's t distribution with `df` degrees of freedom.
#[inline]
pub fn student_t_two_tailed_p_value(t: f64, df: f64) -> f64 {
    if df <= 0.0 {
        return 1.0;
    }
    let x = df / (df + t * t);
    inc_beta_reg(x, df * 0.5, 0.5).clamp(0.0, 1.0)
}

/// Computes the critical t-value for a two-tailed test at significance level `alpha` and `df` degrees of freedom.
pub fn student_t_critical_value(df: f64, alpha: f64) -> f64 {
    if df <= 0.0 || alpha <= 0.0 || alpha >= 1.0 {
        return 1.95996;
    }
    let z = inverse_normal_cdf(1.0 - alpha * 0.5);
    let t0 = z + (z.powi(3) + z) / (4.0 * df) + (5.0 * z.powi(5) + 16.0 * z.powi(3) + 3.0 * z) / (96.0 * df * df);
    let mut low = (t0 * 0.5).max(0.01);
    let mut high = t0 * 2.0 + 2.0;
    let mut best_t = t0;

    for _ in 0..40 {
        let mid = 0.5 * (low + high);
        let p_mid = student_t_two_tailed_p_value(mid, df);
        if (p_mid - alpha).abs() < 1e-12 {
            return mid;
        }
        if p_mid > alpha {
            low = mid;
        } else {
            high = mid;
        }
        best_t = mid;
    }
    best_t
}

// ============================================================================
// Hampel Filter (Robust Outlier Rejection via MAD)
// ============================================================================

/// Result of Hampel filtering with MAD metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HampelFilterResult {
    pub cleaned: Vec<f64>,
    pub outliers: Vec<f64>,
    pub median: f64,
    pub mad: f64,
    pub sigma: f64,
}

/// Hampel 3-sigma robust outlier filter based on Median Absolute Deviation.
#[derive(Debug, Clone, Copy)]
pub struct HampelFilter {
    pub threshold_k: f64,
}

impl Default for HampelFilter {
    fn default() -> Self {
        Self { threshold_k: 3.0 }
    }
}

impl HampelFilter {
    /// Creates a new Hampel filter with threshold multiplier `k`.
    pub fn new(threshold_k: f64) -> Self {
        Self { threshold_k }
    }

    /// Computes the median of a slice.
    pub fn calc_median(data: &[f64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let len = sorted.len();
        if len % 2 == 1 {
            sorted[len / 2]
        } else {
            0.5 * (sorted[len / 2 - 1] + sorted[len / 2])
        }
    }

    /// Filters dataset, separating inliers from outliers using MAD.
    pub fn filter(&self, data: &[f64]) -> HampelFilterResult {
        if data.len() < 3 {
            return HampelFilterResult {
                cleaned: data.to_vec(),
                outliers: Vec::new(),
                median: Self::calc_median(data),
                mad: 0.0,
                sigma: 0.0,
            };
        }

        let median = Self::calc_median(data);
        let abs_deviations: Vec<f64> = data.iter().map(|&x| (x - median).abs()).collect();
        let mad = Self::calc_median(&abs_deviations);
        let sigma = 1.482602218505602 * mad;
        let cutoff = self.threshold_k * sigma;

        if sigma <= 1e-12 {
            return HampelFilterResult {
                cleaned: data.to_vec(),
                outliers: Vec::new(),
                median,
                mad,
                sigma,
            };
        }

        let mut cleaned = Vec::with_capacity(data.len());
        let mut outliers = Vec::new();
        for &val in data {
            if (val - median).abs() <= cutoff {
                cleaned.push(val);
            } else {
                outliers.push(val);
            }
        }

        if cleaned.len() < 2 {
            cleaned = data.to_vec();
            outliers.clear();
        }

        HampelFilterResult { cleaned, outliers, median, mad, sigma }
    }
}

// ============================================================================
// Welch's Heteroscedastic Two-Sample t-Test & Confidence Intervals
// ============================================================================

/// Result of Welch's two-sample t-test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WelchTTestResult {
    pub t_statistic: f64,
    pub degrees_of_freedom: f64,
    pub p_value: f64,
    pub mean_diff: f64,
    pub std_error_diff: f64,
    pub alpha: f64,
}

/// Two-sided confidence interval for differences and ratios.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceInterval {
    pub confidence_level: f64,
    pub point_estimate: f64,
    pub lower: f64,
    pub upper: f64,
    pub margin_of_error: f64,
}

/// Decision verdict based on Welch's t-test and 95% confidence interval boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionVerdict {
    SignificantSpeedup,
    SignificantRegression,
    NeutralNoise,
}

/// Welch's Student t-test engine.
pub struct WelchStudentTTest;

impl WelchStudentTTest {
    /// Computes sample mean and sample variance (unbiased $n-1$).
    pub fn sample_mean_and_variance(data: &[f64]) -> (f64, f64) {
        let n = data.len();
        if n == 0 {
            return (0.0, 0.0);
        }
        if n == 1 {
            return (data[0], 0.0);
        }
        let mean = data.iter().sum::<f64>() / n as f64;
        let var = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
        (mean, var)
    }

    /// Performs Welch's two-sample t-test comparing baseline A against candidate B.
    pub fn test(sample_a: &[f64], sample_b: &[f64], alpha: f64) -> WelchTTestResult {
        let (mean_a, var_a) = Self::sample_mean_and_variance(sample_a);
        let (mean_b, var_b) = Self::sample_mean_and_variance(sample_b);
        let n_a = sample_a.len() as f64;
        let n_b = sample_b.len() as f64;

        if n_a < 2.0 || n_b < 2.0 {
            return WelchTTestResult {
                t_statistic: 0.0,
                degrees_of_freedom: 1.0,
                p_value: 1.0,
                mean_diff: mean_b - mean_a,
                std_error_diff: 0.0,
                alpha,
            };
        }

        let u_a = var_a / n_a;
        let u_b = var_b / n_b;
        let se_diff = (u_a + u_b).sqrt();

        if se_diff <= 1e-15 {
            return WelchTTestResult {
                t_statistic: 0.0,
                degrees_of_freedom: n_a + n_b - 2.0,
                p_value: 1.0,
                mean_diff: mean_b - mean_a,
                std_error_diff: 0.0,
                alpha,
            };
        }

        let denom_df = (u_a * u_a / (n_a - 1.0)) + (u_b * u_b / (n_b - 1.0));
        let df = if denom_df > 1e-15 { ((u_a + u_b) * (u_a + u_b)) / denom_df } else { n_a + n_b - 2.0 }.max(1.0);
        let t_stat = (mean_b - mean_a) / se_diff;
        let p_val = student_t_two_tailed_p_value(t_stat.abs(), df);

        WelchTTestResult {
            t_statistic: t_stat,
            degrees_of_freedom: df,
            p_value: p_val,
            mean_diff: mean_b - mean_a,
            std_error_diff: se_diff,
            alpha,
        }
    }
}

// ============================================================================
// Measurement Engine & Stats Aggregator
// ============================================================================

/// Configuration for the adaptive measurement loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementConfig {
    pub warmup_iterations: usize,
    pub min_iterations: usize,
    pub max_iterations: usize,
    pub target_rse_pct: f64,
    pub enable_hampel: bool,
    pub hampel_k: f64,
}

impl Default for MeasurementConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 5,
            min_iterations: 10,
            max_iterations: 100,
            target_rse_pct: 0.5,
            enable_hampel: true,
            hampel_k: 3.0,
        }
    }
}

/// Statistical evaluation of benchmark measurement samples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementStats {
    pub sample_count: usize,
    pub outliers_count: usize,
    pub mean_nanos: f64,
    pub median_nanos: f64,
    pub std_dev_nanos: f64,
    pub mad_nanos: f64,
    pub rse_pct: f64,
    pub min_nanos: f64,
    pub max_nanos: f64,
    pub converged: bool,
    pub raw_samples: Vec<f64>,
    pub clean_samples: Vec<f64>,
}

/// Comprehensive A/B comparison metrics and decision verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonStats {
    pub baseline: MeasurementStats,
    pub candidate: MeasurementStats,
    pub delta_mean_nanos: f64,
    pub delta_pct: f64,
    pub speedup_ratio: f64,
    pub delta_pct_ci: ConfidenceInterval,
    pub speedup_ci: ConfidenceInterval,
    pub t_test: WelchTTestResult,
    pub verdict: DecisionVerdict,
}

/// Adaptive benchmark measurement engine.
#[derive(Debug, Clone)]
pub struct MeasurementEngine {
    pub config: MeasurementConfig,
}

impl Default for MeasurementEngine {
    fn default() -> Self {
        Self::new(MeasurementConfig::default())
    }
}

impl MeasurementEngine {
    /// Creates a measurement engine with specific configuration.
    pub fn new(config: MeasurementConfig) -> Self {
        Self { config }
    }

    /// Evaluates raw timing samples (in nanoseconds) into `MeasurementStats`.
    pub fn compute_stats(&self, raw_samples: &[f64], converged: bool) -> MeasurementStats {
        if raw_samples.is_empty() {
            return MeasurementStats {
                sample_count: 0,
                outliers_count: 0,
                mean_nanos: 0.0,
                median_nanos: 0.0,
                std_dev_nanos: 0.0,
                mad_nanos: 0.0,
                rse_pct: 0.0,
                min_nanos: 0.0,
                max_nanos: 0.0,
                converged: false,
                raw_samples: Vec::new(),
                clean_samples: Vec::new(),
            };
        }

        let hampel_res = if self.config.enable_hampel && raw_samples.len() >= 3 {
            HampelFilter::new(self.config.hampel_k).filter(raw_samples)
        } else {
            HampelFilterResult {
                cleaned: raw_samples.to_vec(),
                outliers: Vec::new(),
                median: HampelFilter::calc_median(raw_samples),
                mad: 0.0,
                sigma: 0.0,
            }
        };

        let clean = &hampel_res.cleaned;
        let (mean, var) = WelchStudentTTest::sample_mean_and_variance(clean);
        let std_dev = var.sqrt();
        let se = if clean.is_empty() { 0.0 } else { std_dev / (clean.len() as f64).sqrt() };
        let rse_pct = if mean > 1e-15 { (se / mean) * 100.0 } else { 0.0 };

        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;
        for &val in clean {
            if val < min_val { min_val = val; }
            if val > max_val { max_val = val; }
        }
        if clean.is_empty() { min_val = 0.0; max_val = 0.0; }

        MeasurementStats {
            sample_count: raw_samples.len(),
            outliers_count: hampel_res.outliers.len(),
            mean_nanos: mean,
            median_nanos: hampel_res.median,
            std_dev_nanos: std_dev,
            mad_nanos: hampel_res.mad,
            rse_pct,
            min_nanos: min_val,
            max_nanos: max_val,
            converged,
            raw_samples: raw_samples.to_vec(),
            clean_samples: hampel_res.cleaned,
        }
    }

    /// Measures execution of a closure adaptively until target RSE is achieved or max iterations reached.
    pub fn measure<F: FnMut()>(&self, mut f: F) -> MeasurementStats {
        for _ in 0..self.config.warmup_iterations {
            f();
        }

        let mut raw_samples = Vec::with_capacity(self.config.max_iterations);
        let mut converged = false;

        for i in 1..=self.config.max_iterations {
            let start = sync_to_next_tick();
            f();
            let elapsed_nanos = start.elapsed().as_nanos() as f64;
            raw_samples.push(elapsed_nanos);

            if i >= self.config.min_iterations {
                let stats = self.compute_stats(&raw_samples, false);
                if stats.rse_pct <= self.config.target_rse_pct && stats.clean_samples.len() >= self.config.min_iterations {
                    converged = true;
                    break;
                }
            }
        }

        self.compute_stats(&raw_samples, converged)
    }

    /// Compares baseline measurement against candidate measurement, yielding statistical comparison.
    pub fn compare(baseline: MeasurementStats, candidate: MeasurementStats, alpha: f64) -> ComparisonStats {
        let t_test = WelchStudentTTest::test(&baseline.clean_samples, &candidate.clean_samples, alpha);
        let mean_base = baseline.mean_nanos;
        let mean_cand = candidate.mean_nanos;
        let delta_mean = mean_cand - mean_base;
        let delta_pct = if mean_base > 1e-15 { (delta_mean / mean_base) * 100.0 } else { 0.0 };
        let speedup = if mean_cand > 1e-15 { mean_base / mean_cand } else { 1.0 };

        let t_crit = student_t_critical_value(t_test.degrees_of_freedom, alpha);
        let moe_diff = t_crit * t_test.std_error_diff;
        let moe_pct = if mean_base > 1e-15 { (moe_diff / mean_base) * 100.0 } else { 0.0 };

        let delta_pct_ci = ConfidenceInterval {
            confidence_level: 1.0 - alpha,
            point_estimate: delta_pct,
            lower: delta_pct - moe_pct,
            upper: delta_pct + moe_pct,
            margin_of_error: moe_pct,
        };

        // Log-ratio delta method for speedup ratio CI
        let rse_base_frac = baseline.rse_pct / 100.0;
        let rse_cand_frac = candidate.rse_pct / 100.0;
        let log_ratio_se = (rse_base_frac.powi(2) + rse_cand_frac.powi(2)).sqrt();
        let log_ratio_moe = t_crit * log_ratio_se;
        let speedup_lower = speedup * (-log_ratio_moe).exp();
        let speedup_upper = speedup * log_ratio_moe.exp();

        let speedup_ci = ConfidenceInterval {
            confidence_level: 1.0 - alpha,
            point_estimate: speedup,
            lower: speedup_lower,
            upper: speedup_upper,
            margin_of_error: (speedup_upper - speedup_lower) * 0.5,
        };

        let verdict = if t_test.p_value < alpha {
            if delta_pct_ci.upper < 0.0 {
                DecisionVerdict::SignificantSpeedup
            } else if delta_pct_ci.lower > 0.0 {
                DecisionVerdict::SignificantRegression
            } else {
                DecisionVerdict::NeutralNoise
            }
        } else {
            DecisionVerdict::NeutralNoise
        };

        ComparisonStats {
            baseline,
            candidate,
            delta_mean_nanos: delta_mean,
            delta_pct,
            speedup_ratio: speedup,
            delta_pct_ci,
            speedup_ci,
            t_test,
            verdict,
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lanczos_lgamma_known_values() {
        assert!((lanczos_lgamma(1.0) - 0.0).abs() < 1e-10);
        assert!((lanczos_lgamma(2.0) - 0.0).abs() < 1e-10);
        assert!((lanczos_lgamma(3.0) - 2.0_f64.ln()).abs() < 1e-10);
        assert!((lanczos_lgamma(5.0) - 24.0_f64.ln()).abs() < 1e-10);
        assert!((lanczos_lgamma(0.5) - PI.sqrt().ln()).abs() < 1e-10);
    }

    #[test]
    fn test_incomplete_beta_boundaries() {
        assert_eq!(inc_beta_reg(0.0, 2.0, 3.0), 0.0);
        assert_eq!(inc_beta_reg(1.0, 2.0, 3.0), 1.0);
        let val = inc_beta_reg(0.5, 1.0, 1.0);
        assert!((val - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_student_t_critical_values() {
        let t_95_inf = student_t_critical_value(1000.0, 0.05);
        assert!((t_95_inf - 1.96).abs() < 0.02);

        let t_95_df10 = student_t_critical_value(10.0, 0.05);
        assert!((t_95_df10 - 2.228).abs() < 0.01);
    }

    #[test]
    fn test_hampel_filter_removes_outliers() {
        let filter = HampelFilter::default();
        let data = vec![100.0, 100.1, 99.9, 100.05, 99.95, 100.0, 500.0, 100.1, 99.9, 0.0];
        let res = filter.filter(&data);

        assert_eq!(res.outliers.len(), 2);
        assert!(res.outliers.contains(&500.0));
        assert!(res.outliers.contains(&0.0));
        assert_eq!(res.cleaned.len(), 8);
        assert!((res.median - 100.025).abs() < 0.2);
    }

    #[test]
    fn test_hampel_filter_identical_data() {
        let filter = HampelFilter::default();
        let data = vec![50.0; 10];
        let res = filter.filter(&data);
        assert_eq!(res.cleaned.len(), 10);
        assert_eq!(res.outliers.len(), 0);
    }

    #[test]
    fn test_welch_t_test_significant_difference() {
        let base = vec![100.0, 101.0, 102.0, 99.0, 100.5, 101.5];
        let cand = vec![80.0, 81.0, 79.5, 80.5, 81.5, 79.0];

        let result = WelchStudentTTest::test(&base, &cand, 0.05);
        assert!(result.p_value < 0.0001);
        assert!(result.t_statistic < -10.0);
    }

    #[test]
    fn test_welch_t_test_identical_samples() {
        let sample = vec![10.0, 11.0, 9.0, 10.5, 9.5];
        let result = WelchStudentTTest::test(&sample, &sample, 0.05);
        assert_eq!(result.mean_diff, 0.0);
        assert_eq!(result.p_value, 1.0);
    }

    #[test]
    fn test_measurement_engine_convergence_and_verdict() {
        let engine = MeasurementEngine::new(MeasurementConfig {
            warmup_iterations: 2,
            min_iterations: 10,
            max_iterations: 50,
            target_rse_pct: 1.0,
            enable_hampel: true,
            hampel_k: 3.0,
        });

        let base_raw = vec![100.0, 100.2, 99.8, 100.1, 99.9, 100.05, 99.95, 100.1, 100.0, 99.9];
        let cand_raw = vec![70.0, 70.2, 69.8, 70.1, 69.9, 70.05, 69.95, 70.1, 70.0, 69.9];

        let base_stats = engine.compute_stats(&base_raw, true);
        let cand_stats = engine.compute_stats(&cand_raw, true);

        assert!(base_stats.rse_pct < 1.0);
        assert_eq!(base_stats.outliers_count, 0);

        let comp = MeasurementEngine::compare(base_stats, cand_stats, 0.05);
        assert_eq!(comp.verdict, DecisionVerdict::SignificantSpeedup);
        assert!(comp.speedup_ratio > 1.4);
        assert!(comp.delta_pct < -25.0);
    }

    #[test]
    fn test_sync_to_next_tick() {
        let t1 = sync_to_next_tick();
        let t2 = sync_to_next_tick();
        assert!(t2 >= t1);
    }
}
