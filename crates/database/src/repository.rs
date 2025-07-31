use crate::DbError;
use analytics::PerformanceReport;
use chrono::{DateTime, Utc};
use core_types::{Kline, Trade};
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use sqlx::postgres::PgPool;
use sqlx::postgres::Postgres;
use sqlx::Transaction;
use uuid::Uuid;

/// The `DbRepository` provides a high-level, application-specific interface
/// to the database. It encapsulates all SQL queries and data access logic.
#[derive(Debug, Clone)]
pub struct DbRepository {
    pool: PgPool,
}

impl DbRepository {
    /// Creates a new `DbRepository` with a shared database connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Saves a single Kline to the database.
    /// Uses `ON CONFLICT DO NOTHING` to be idempotent, so it can be called repeatedly
    /// without causing errors if the data already exists.
    pub async fn save_kline(&self, kline: &Kline) -> Result<(), DbError> {
        sqlx::query!(
            r#"
            INSERT INTO klines (symbol, interval, open_time, open, high, low, close, volume)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (symbol, interval, open_time) DO NOTHING
            "#,
            "BTC/USDT", // Placeholder until symbol management is more robust
            kline.interval,
            kline.open_time,
            &kline.open,
            &kline.high,
            &kline.low,
            &kline.close,
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
}