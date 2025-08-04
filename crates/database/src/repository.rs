use crate::DbError;
use analytics::PerformanceReport;
use chrono::{DateTime, Utc};
use core_types::{Kline, Trade, Execution, OrderSide};
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use sqlx::postgres::PgPool;
use sqlx::postgres::Postgres;
use sqlx::Row;
use serde::{Deserialize, Serialize};
use sqlx::Transaction;
use uuid::Uuid;
use sqlx::FromRow;
/// The `DbRepository` provides a high-level, application-specific interface
/// to the database. It encapsulates all SQL queries and data access logic.
#[derive(Debug, Clone)]
pub struct DbRepository {
    pool: PgPool,
}

// Define a simple struct for an equity curve point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityDataPoint {
    pub timestamp: DateTime<Utc>,
    pub equity: Decimal,
}

// This struct will hold all the data for the details page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestRunDetails {
    pub report: FullReport,
    pub trades: Vec<Trade>,
    pub equity_curve: Vec<EquityDataPoint>,
}
// This struct represents a row fetched from the backtest_runs table.
#[derive(FromRow, Debug, Clone)]
pub struct DbBacktestRun {
    pub run_id: Uuid,
    pub job_id: Uuid,
    pub parameters: JsonValue,
    pub run_status: String,
}
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbOptimizationJob {
    pub job_id: Uuid,
    pub strategy_id: String,
    pub symbol: String,
    pub job_status: String,
    pub created_at: DateTime<Utc>,
}
/// Represents a row from the `wfo_jobs` table.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WfoJob {
    pub wfo_job_id: Uuid,
    pub strategy_id: String,
    pub symbol: String,
    pub in_sample_period_months: i32,
    pub out_of_sample_period_months: i32,
    pub wfo_status: String,
    pub created_at: DateTime<Utc>,
}

/// Represents a row from the `wfo_runs` table, detailing a single OOS run.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WfoRun {
    pub wfo_run_id: Uuid,
    pub wfo_job_id: Uuid,
    pub oos_run_id: Uuid,
    pub best_in_sample_parameters: JsonValue,
    pub oos_start_date: DateTime<Utc>,
    pub oos_end_date: DateTime<Utc>,
}
/// A struct that represents the result of joining `performance_reports`
/// with `backtest_runs` to get a complete picture of a single run.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FullReport {
    // Fields from backtest_runs
    pub run_id: Uuid,
    pub job_id: Uuid,
    pub parameters: JsonValue,
    
    // Fields from performance_reports
    pub report_id: Option<Uuid>,
    pub total_net_profit: Option<Decimal>,
    pub gross_profit: Option<Decimal>,
    pub gross_loss: Option<Decimal>,
    pub profit_factor: Option<Decimal>,
    pub total_return_pct: Option<Decimal>,
    pub max_drawdown: Option<Decimal>,
    pub max_drawdown_pct: Option<Decimal>,
    pub sharpe_ratio: Option<Decimal>,
    pub calmar_ratio: Option<Decimal>,
    pub total_trades: Option<i32>,
    pub winning_trades: Option<i32>,
    pub losing_trades: Option<i32>,
    pub win_rate_pct: Option<Decimal>,
    pub average_win: Option<Decimal>,
    pub average_loss: Option<Decimal>,
    pub payoff_ratio: Option<Decimal>,
    pub average_holding_period: Option<String>,
}

/// Database-specific trade struct that matches the trades table schema
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbTrade {
    pub trade_id: Uuid,
    pub run_id: Uuid,
    pub symbol: String,
    pub entry_price: Decimal,
    pub entry_qty: Decimal,
    pub entry_timestamp: DateTime<Utc>,
    pub exit_price: Decimal,
    pub exit_qty: Decimal,
    pub exit_timestamp: DateTime<Utc>,
}
impl DbRepository {
    /// Creates a new `DbRepository` with a shared database connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Fetches all optimization jobs from the database.
    /// In a real app, this would support pagination with OFFSET and LIMIT.
    pub async fn get_all_optimization_jobs(&self) -> Result<Vec<DbOptimizationJob>, DbError> {
        let jobs = sqlx::query_as!(
            DbOptimizationJob,
            "SELECT job_id, strategy_id, symbol, job_status, created_at FROM optimization_jobs ORDER BY created_at DESC"
        ).fetch_all(&self.pool).await?;
        Ok(jobs)
    }
    /// Fetches all backtest runs that were executed as 'Single Run' jobs.
    /// This joins with the performance report to provide a useful summary.
    pub async fn get_all_single_runs(&self) -> Result<Vec<FullReport>, DbError> {
        let reports = sqlx::query_as!(
            FullReport,
            r#"
            SELECT
                br.run_id as "run_id!", br.job_id as "job_id!", br.parameters as "parameters!", pr.report_id as "report_id?", pr.total_net_profit as "total_net_profit?", pr.gross_profit as "gross_profit?", pr.gross_loss as "gross_loss?", pr.profit_factor as "profit_factor?", pr.total_return_pct as "total_return_pct?", pr.max_drawdown as "max_drawdown?", pr.max_drawdown_pct as "max_drawdown_pct?", pr.sharpe_ratio as "sharpe_ratio?", pr.calmar_ratio as "calmar_ratio?", pr.total_trades as "total_trades?", pr.winning_trades as "winning_trades?", pr.losing_trades as "losing_trades?", pr.win_rate_pct as "win_rate_pct?", pr.average_win as "average_win?", pr.average_loss as "average_loss?", pr.payoff_ratio as "payoff_ratio?", pr.average_holding_period as "average_holding_period?"
            FROM
                performance_reports AS pr
            JOIN
                backtest_runs AS br ON pr.run_id = br.run_id
            JOIN
                optimization_jobs AS oj ON br.job_id = oj.job_id
            WHERE
                oj.job_status = 'Single Run'
            ORDER BY
                oj.created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(reports)
    }

    /// Fetches the full, joined report for a single backtest run ID.
    pub async fn get_full_report_for_run(&self, run_id: Uuid) -> Result<FullReport, DbError> {
        let report = sqlx::query_as!(
            FullReport,
            r#"
            SELECT
                br.run_id as "run_id!", br.job_id as "job_id!", br.parameters as "parameters!", pr.report_id as "report_id?", pr.total_net_profit as "total_net_profit?", pr.gross_profit as "gross_profit?", pr.gross_loss as "gross_loss?", pr.profit_factor as "profit_factor?", pr.total_return_pct as "total_return_pct?", pr.max_drawdown as "max_drawdown?", pr.max_drawdown_pct as "max_drawdown_pct?", pr.sharpe_ratio as "sharpe_ratio?", pr.calmar_ratio as "calmar_ratio?", pr.total_trades as "total_trades?", pr.winning_trades as "winning_trades?", pr.losing_trades as "losing_trades?", pr.win_rate_pct as "win_rate_pct?", pr.average_win as "average_win?", pr.average_loss as "average_loss?", pr.payoff_ratio as "payoff_ratio?", pr.average_holding_period as "average_holding_period?"
            FROM
                performance_reports AS pr
            JOIN
                backtest_runs AS br ON pr.run_id = br.run_id
            WHERE
                br.run_id = $1
            "#,
            run_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| if let sqlx::Error::RowNotFound = e { DbError::NotFound } else { e.into() })?;
        
        Ok(report)
    }
    /// Fetches all klines for a given symbol and interval within a date range.
    pub async fn get_klines_by_date_range(
        &self,
        symbol: &str,
        interval: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<Kline>, DbError> {
        let rows = sqlx::query(
            r#"
            SELECT open_time, open, high, low, close, volume, close_time
            FROM klines
            WHERE symbol = $1 AND interval = $2 AND open_time >= $3 AND open_time <= $4
            ORDER BY open_time ASC
            "#,
        )
        .bind(symbol)
        .bind(interval)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        let klines = rows.into_iter().map(|row| {
            Kline {
                open_time: row.get("open_time"),
                open: row.get("open"),
                high: row.get("high"),
                low: row.get("low"),
                close: row.get("close"),
                volume: row.get("volume"),
                close_time: row.get("close_time"),
                interval: interval.to_string(),
            }
        }).collect();

        Ok(klines)
    }

     /// Fetches all backtest runs for a given job that have a 'Pending' status.
     pub async fn get_pending_runs(&self, job_id: Uuid) -> Result<Vec<DbBacktestRun>, DbError> {
        let runs = sqlx::query_as::<_, DbBacktestRun>(
            "SELECT run_id, job_id, parameters, run_status FROM backtest_runs WHERE job_id = $1 AND run_status = 'Pending'"
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(runs)
    }

    /// Updates the status of a specific backtest run.
    pub async fn update_run_status(&self, run_id: Uuid, status: &str) -> Result<(), DbError> {
        sqlx::query("UPDATE backtest_runs SET run_status = $1 WHERE run_id = $2")
            .bind(status)
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Saves a single Kline to the database.
    /// Uses `ON CONFLICT DO NOTHING` to be idempotent, so it can be called repeatedly
    /// without causing errors if the data already exists.
    pub async fn save_kline(&self, symbol: &str, kline: &Kline) -> Result<(), DbError> { // <-- MODIFIED SIGNATURE
        sqlx::query!(
            r#"
            INSERT INTO klines (symbol, interval, open_time, close_time, open, high, low, close, volume)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (symbol, interval, open_time) DO NOTHING
            "#,
            symbol,
            kline.interval,
            kline.open_time,
            kline.close_time,
            kline.open,
            kline.high,
            kline.low,
            kline.close,
            kline.volume
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Creates a new record for a top-level optimization job.
    pub async fn save_optimization_job(
        &self,
        job_id: Uuid,
        strategy_id: &str,
        symbol: &str,
        status: &str,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "INSERT INTO optimization_jobs (job_id, strategy_id, symbol, job_status, created_at) VALUES ($1, $2, $3, $4, NOW())",
            job_id,
            strategy_id,
            symbol,
            status
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Saves a record for a single backtest run, linked to an optimization job.
    pub async fn save_backtest_run(
        &self,
        run_id: Uuid,
        job_id: Uuid,
        parameters: &JsonValue,
        status: &str,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "INSERT INTO backtest_runs (run_id, job_id, parameters, run_status, created_at) VALUES ($1, $2, $3, $4, NOW())",
            run_id,
            job_id,
            parameters,
            status
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    
    
    /// Saves a full performance report linked to a specific backtest run.
    pub async fn save_performance_report(
        &self,
        run_id: Uuid,
        report: &PerformanceReport,
    ) -> Result<(), DbError> {
        // The `humantime` crate is the standard for this, but to_string() is sufficient for now.
        let avg_holding_period_str = report.average_holding_period.to_string();

        // Prepare and execute the query with all parameters
        let query = r#"
            INSERT INTO performance_reports (
                run_id, total_net_profit, gross_profit, gross_loss, profit_factor,
                total_return_pct, max_drawdown, max_drawdown_pct, sharpe_ratio,
                calmar_ratio, total_trades, winning_trades, losing_trades,
                win_rate_pct, average_win, average_loss, payoff_ratio, average_holding_period
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
            )
            "#;
            
        sqlx::query(query)
            .bind(run_id)
            .bind(&report.total_net_profit)  // Decimal
            .bind(&report.gross_profit)      // Decimal
            .bind(&report.gross_loss)        // Decimal
            .bind(report.profit_factor.as_ref())  // Option<Decimal>
            .bind(&report.total_return_pct)  // Decimal
            .bind(&report.max_drawdown)      // Decimal
            .bind(&report.max_drawdown_pct)  // Decimal
            .bind(report.sharpe_ratio.as_ref())   // Option<Decimal>
            .bind(report.calmar_ratio.as_ref())   // Option<Decimal>
            .bind(report.total_trades as i32)     // i32
            .bind(report.winning_trades as i32)   // i32
            .bind(report.losing_trades as i32)    // i32
            .bind(report.win_rate_pct.as_ref())   // Option<Decimal>
            .bind(&report.average_win)      // Decimal
            .bind(&report.average_loss)      // Decimal
            .bind(report.payoff_ratio.as_ref())   // Option<Decimal>
            .bind(avg_holding_period_str)    // String
            .execute(&self.pool)
            .await?;
            
        Ok(())
    }

    /// Saves a batch of trades from a backtest run within a single transaction for atomicity.
    pub async fn save_trades(&self, run_id: Uuid, trades: &[Trade]) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;

        for trade in trades {
            sqlx::query!(
                r#"
                INSERT INTO trades (
                    trade_id, run_id, symbol, entry_price, entry_qty, entry_timestamp,
                    exit_price, exit_qty, exit_timestamp
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                trade.trade_id,
                run_id,
                trade.symbol,
                &trade.entry_execution.price,
                &trade.entry_execution.quantity,
                trade.entry_execution.timestamp,
                &trade.exit_execution.price,
                &trade.exit_execution.quantity,
                trade.exit_execution.timestamp
            )
            .execute(&mut *tx) // Note: must use the transaction object `tx` here
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Fetches a complete set of reports for a given optimization job, joining
    /// backtest run data (for parameters) with performance report data (for results).
    pub async fn get_full_reports_for_job(&self, job_id: Uuid) -> Result<Vec<FullReport>, DbError> {
        let reports = sqlx::query_as!(
            FullReport,
            r#"
            SELECT
                br.run_id as "run_id!", br.job_id as "job_id!", br.parameters as "parameters!", pr.report_id as "report_id?", pr.total_net_profit as "total_net_profit?", pr.gross_profit as "gross_profit?", pr.gross_loss as "gross_loss?", pr.profit_factor as "profit_factor?", pr.total_return_pct as "total_return_pct?", pr.max_drawdown as "max_drawdown?", pr.max_drawdown_pct as "max_drawdown_pct?", pr.sharpe_ratio as "sharpe_ratio?", pr.calmar_ratio as "calmar_ratio?", pr.total_trades as "total_trades?", pr.winning_trades as "winning_trades?", pr.losing_trades as "losing_trades?", pr.win_rate_pct as "win_rate_pct?", pr.average_win as "average_win?", pr.average_loss as "average_loss?", pr.payoff_ratio as "payoff_ratio?", pr.average_holding_period as "average_holding_period?"
            FROM
                performance_reports AS pr
            JOIN
                backtest_runs AS br ON pr.run_id = br.run_id
            WHERE
                br.job_id = $1
            "#,
            job_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(reports)
    }
    /// Saves the full equity curve for a backtest run within a single transaction.
    pub async fn save_equity_curve(
        &self,
        run_id: Uuid,
        equity_curve: &[(DateTime<Utc>, Decimal)],
    ) -> Result<(), DbError> {
        let mut tx: Transaction<Postgres> = self.pool.begin().await?;

        for (timestamp, equity) in equity_curve {
            sqlx::query!(
                "INSERT INTO equity_curves (run_id, timestamp, equity) VALUES ($1, $2, $3)",
                run_id,
                timestamp,
                equity
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
    /// Creates a new top-level record for a Walk-Forward Optimization job.
    pub async fn save_wfo_job(
        &self,
        wfo_job_id: Uuid,
        strategy_id: &str,
        symbol: &str,
        in_sample_period_months: i32,
        out_of_sample_period_months: i32,
        wfo_status: &str,
    ) -> Result<(), DbError> {
        sqlx::query!(
            r#"
            INSERT INTO wfo_jobs (wfo_job_id, strategy_id, symbol, in_sample_period_months, out_of_sample_period_months, wfo_status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
            wfo_job_id,
            strategy_id,
            symbol,
            in_sample_period_months,
            out_of_sample_period_months,
            wfo_status
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Saves the record of a single, completed out-of-sample run within a WFO job.
    pub async fn save_wfo_run(
        &self,
        wfo_run_id: Uuid,
        wfo_job_id: Uuid,
        oos_run_id: Uuid,
        best_in_sample_parameters: &JsonValue,
        oos_start_date: DateTime<Utc>,
        oos_end_date: DateTime<Utc>,
    ) -> Result<(), DbError> {
        sqlx::query!(
            r#"
            INSERT INTO wfo_runs (wfo_run_id, wfo_job_id, oos_run_id, best_in_sample_parameters, oos_start_date, oos_end_date)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            wfo_run_id,
            wfo_job_id,
            oos_run_id,
            best_in_sample_parameters,
            oos_start_date,
            oos_end_date
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    pub async fn get_run_details(&self, run_id: Uuid) -> Result<BacktestRunDetails, DbError> {
        let report_future = self.get_full_report_for_run(run_id);
        
        let trades_future = sqlx::query_as!(
            DbTrade,
            r#"SELECT trade_id, run_id, symbol, entry_price, entry_qty, entry_timestamp, exit_price, exit_qty, exit_timestamp FROM trades WHERE run_id = $1 ORDER BY entry_timestamp ASC"#,
            run_id
        ).fetch_all(&self.pool);

        let equity_curve_future = sqlx::query_as!(
            EquityDataPoint,
            "SELECT timestamp, equity FROM equity_curves WHERE run_id = $1 ORDER BY timestamp ASC",
            run_id
        ).fetch_all(&self.pool);

        let (report_res, trades_res, equity_curve_res) = tokio::join!(report_future, trades_future, equity_curve_future);

        // Convert DbTrade to Trade (core_types)
        let trades: Vec<Trade> = trades_res?.into_iter().map(|db_trade| {
            let entry_execution = Execution {
                execution_id: Uuid::new_v4(), // Generate new ID since we don't store it in DB
                client_order_id: Uuid::new_v4(), // Generate new ID since we don't store it in DB
                symbol: db_trade.symbol.clone(),
                side: OrderSide::Buy, // We'll need to determine this from context
                price: db_trade.entry_price,
                quantity: db_trade.entry_qty,
                fee: Decimal::ZERO, // Not stored in DB
                fee_asset: String::new(), // Not stored in DB
                timestamp: db_trade.entry_timestamp,
            };
            
            let exit_execution = Execution {
                execution_id: Uuid::new_v4(), // Generate new ID since we don't store it in DB
                client_order_id: Uuid::new_v4(), // Generate new ID since we don't store it in DB
                symbol: db_trade.symbol.clone(),
                side: OrderSide::Sell, // We'll need to determine this from context
                price: db_trade.exit_price,
                quantity: db_trade.exit_qty,
                fee: Decimal::ZERO, // Not stored in DB
                fee_asset: String::new(), // Not stored in DB
                timestamp: db_trade.exit_timestamp,
            };

            Trade {
                trade_id: db_trade.trade_id,
                symbol: db_trade.symbol,
                entry_execution,
                exit_execution,
            }
        }).collect();

        Ok(BacktestRunDetails {
            report: report_res?,
            trades,
            equity_curve: equity_curve_res?,
        })
    }

    /// Fetches all WFO jobs from the database.
    pub async fn get_all_wfo_jobs(&self) -> Result<Vec<WfoJob>, DbError> {
        let jobs = sqlx::query_as!(
            WfoJob,
            "SELECT wfo_job_id, strategy_id, symbol, in_sample_period_months, out_of_sample_period_months, wfo_status, created_at FROM wfo_jobs ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(jobs)
    }

    /// Fetches all WFO runs for a specific WFO job.
    pub async fn get_wfo_runs_for_job(&self, wfo_job_id: Uuid) -> Result<Vec<WfoRun>, DbError> {
        let runs = sqlx::query_as!(
            WfoRun,
            "SELECT wfo_run_id, wfo_job_id, oos_run_id, best_in_sample_parameters, oos_start_date, oos_end_date FROM wfo_runs WHERE wfo_job_id = $1 ORDER BY oos_start_date ASC",
            wfo_job_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(runs)
    }
}