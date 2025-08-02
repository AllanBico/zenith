use crate::error::AnalyzerError;
use configuration::optimizer_config::AnalysisConfig;
use database::DbRepository;
use database::repository::FullReport;
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use uuid::Uuid;

pub mod error;

/// A report that includes the raw performance data, the parameters that produced it,
/// and the final analysis score.
#[derive(Debug, Clone, Serialize)]
pub struct RankedReport {
    pub parameters: Value,
    pub score: Decimal,
    pub report: FullReport,
}

/// The main analysis engine.
pub struct Analyzer {
    config: AnalysisConfig,
}

impl Analyzer {
    pub fn new(config: AnalysisConfig) -> Self {
        Self { config }
    }

    /// Fetches, filters, scores, and ranks all performance reports for a given job.
    pub async fn run(
        &self,
        db_repo: &DbRepository,
        job_id: Uuid,
    ) -> Result<Vec<RankedReport>, AnalyzerError> {
        // 1. Fetch
        let all_reports = db_repo.get_full_reports_for_job(job_id).await?;
        if all_reports.is_empty() {
            return Err(AnalyzerError::NoRunsFound(job_id));
        }

        // 2. Filter
        let filtered_reports = self.filter_reports(all_reports);
        if filtered_reports.is_empty() {
            return Ok(vec![]); // Return empty if all were filtered out
        }

        // 3. Score
        let scored_reports = self.score_reports(filtered_reports)?;

        // 4. Rank
        let mut ranked_reports = scored_reports;
        ranked_reports.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal)
        });

        Ok(ranked_reports)
    }

    /// Applies hard filters to remove unacceptable runs.
    fn filter_reports(&self, reports: Vec<FullReport>) -> Vec<FullReport> {
        reports
            .into_iter()
            .filter(|r| {
                // Safely unwrap or provide default values for Option types
                let total_trades = r.total_trades.unwrap_or(0);
                let max_drawdown_pct = r.max_drawdown_pct.unwrap_or_else(|| Decimal::new(100, 0)); // Default to 100% if None
                
                let passes_trades = total_trades as usize >= self.config.filters.min_total_trades;
                let passes_drawdown = max_drawdown_pct < self.config.filters.max_drawdown_pct;
                
                passes_trades && passes_drawdown
            })
            .collect()
    }
    
    /// Normalizes and applies the weighted scoring function to each report.
    fn score_reports(&self, reports: Vec<FullReport>) -> Result<Vec<RankedReport>, AnalyzerError> {
        // Find min/max for normalization
        let (min_pf, max_pf) = find_min_max(&reports, |r| r.profit_factor);
        let (min_cr, max_cr) = find_min_max(&reports, |r| r.calmar_ratio);
        let (min_pr, max_pr) = find_min_max(&reports, |r| r.payoff_ratio);
        
        reports
            .into_iter()
            .map(|r| {
                let norm_pf = normalize(r.profit_factor.unwrap_or_default(), min_pf, max_pf);
                let norm_cr = normalize(r.calmar_ratio.unwrap_or_default(), min_cr, max_cr);
                let norm_pr = normalize(r.payoff_ratio.unwrap_or_default(), min_pr, max_pr);
                
                let w = &self.config.scoring_weights;
                
                let score = (norm_pf * w.weight_profit_factor)
                          + (norm_cr * w.weight_calmar_ratio)
                          + (norm_pr * w.weight_avg_win_loss_ratio);
                
                Ok(RankedReport {
                    parameters: r.parameters.clone(),
                    score,
                    report: r,
                })
            })
            .collect()
    }
}

/// A helper function to find the min and max of a specific metric in a Vec of reports.
fn find_min_max<F>(reports: &[FullReport], accessor: F) -> (Decimal, Decimal)
where
    F: Fn(&FullReport) -> Option<Decimal>,
{
    reports
        .iter()
        .filter_map(|r| accessor(r))
        .fold((Decimal::MAX, Decimal::MIN), |(min, max), val| {
            (min.min(val), max.max(val))
        })
}

/// Normalizes a value to a 0.0-1.0 scale.
fn normalize(value: Decimal, min: Decimal, max: Decimal) -> Decimal {
    if min == max {
        return Decimal::ONE; // Avoid division by zero if all values are the same
    }
    (value - min) / (max - min)
}