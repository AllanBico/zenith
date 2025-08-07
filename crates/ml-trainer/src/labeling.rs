use anyhow::Result;
use polars::prelude::*;
use rust_decimal::prelude::ToPrimitive;

/// Configuration for the Triple Barrier Method.
pub struct LabelingConfig {
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
    pub time_limit_bars: usize,
}

/// Applies the Triple Barrier Method to kline data.
///
/// This function iterates through each kline and "looks forward"
/// to determine the outcome of a hypothetical trade initiated at that point.
///
/// # Returns
/// A new Series containing the labels:
/// - `1`: Take-profit barrier was hit first.
/// - `-1`: Stop-loss barrier was hit first.
/// - `0`: Time-limit barrier was hit first (a "scratch").
/// Fixed Triple Barrier implementation that works with kline data directly
pub fn apply_triple_barrier_with_klines(
    klines: &[core_types::Kline],
    config: &LabelingConfig,
) -> Result<Series> {
    let num_rows = klines.len();
    let mut labels = Vec::with_capacity(num_rows);

    for i in 0..num_rows {
        let entry_price = klines[i].close.to_f64().unwrap_or(0.0);
        
        if entry_price <= 0.0 {
            labels.push(None);
            continue;
        }
        
        let take_profit_price = entry_price * (1.0 + config.take_profit_pct);
        let stop_loss_price = entry_price * (1.0 - config.stop_loss_pct);
        
        let mut label = None;

        // Look forward in time from the current bar `i`.
        for j in 1..=config.time_limit_bars {
            let future_index = i + j;
            if future_index >= num_rows {
                // We've run out of data before hitting any barrier.
                label = Some(0i32);
                break;
            }

            let future_kline = &klines[future_index];
            let high = future_kline.high.to_f64().unwrap_or(0.0);
            let low = future_kline.low.to_f64().unwrap_or(0.0);
            
            // Check if take profit was hit (using high)
            if high >= take_profit_price {
                label = Some(1i32);
                break;
            }
            // Check if stop loss was hit (using low)  
            if low <= stop_loss_price {
                label = Some(-1i32);
                break;
            }
        }
        
        // If the inner loop finished without hitting TP or SL, it's a timeout.
        if label.is_none() {
            label = Some(0i32);
        }
        
        labels.push(label);
    }
    
    Ok(Series::new("label", labels))
}

// Keep the old function for backwards compatibility but improve it
pub fn apply_triple_barrier(
    df: &DataFrame,
    config: &LabelingConfig,
) -> Result<Series> {
    // This is a fallback implementation when we don't have access to klines
    // We'll use returns but fix the cumulative calculation
    let returns = df.column("returns_1h")?.f64()?;
    
    let num_rows = df.height();
    let mut labels = Vec::with_capacity(num_rows);

    for i in 0..num_rows {
        let entry_return_opt = returns.get(i);
        if entry_return_opt.is_none() {
            labels.push(None);
            continue;
        }
        
        let take_profit_threshold = config.take_profit_pct;
        let stop_loss_threshold = -config.stop_loss_pct;
        
        let mut label = None;
        let mut cumulative_return = 0.0;

        // Look forward in time from the current bar `i`.
        for j in 1..=config.time_limit_bars {
            let future_index = i + j;
            if future_index >= num_rows {
                // We've run out of data before hitting any barrier.
                label = Some(0i32);
                break;
            }

            let future_return_opt = returns.get(future_index);
            if future_return_opt.is_none() {
                continue;
            }
            let future_return = future_return_opt.unwrap();
            
            // FIXED: Properly accumulate returns
            cumulative_return += future_return;
            
            if cumulative_return >= take_profit_threshold {
                label = Some(1i32);
                break;
            }
            if cumulative_return <= stop_loss_threshold {
                label = Some(-1i32);
                break;
            }
        }
        
        // If the inner loop finished without hitting TP or SL, it's a timeout.
        if label.is_none() {
            label = Some(0i32);
        }
        
        labels.push(label);
    }
    
    Ok(Series::new("label", labels))
}