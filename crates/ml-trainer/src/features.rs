use anyhow::Result;
use core_types::Kline;
use polars::prelude::*;
use rust_decimal::prelude::*;
use ta::indicators::{RelativeStrengthIndex as Rsi, MovingAverageConvergenceDivergence as Macd};
use ta::Next;
use chrono::{Timelike, Datelike};

/// Generates a DataFrame of predictive features from a slice of Kline data.
pub fn generate_features(klines: &[Kline]) -> Result<DataFrame> {
    // Convert Vec<Kline> into individual vectors for Polars Series
    let mut closes = Vec::with_capacity(klines.len());
    let mut hours = Vec::with_capacity(klines.len());
    let mut weekdays = Vec::with_capacity(klines.len());
    
    for k in klines {
        closes.push(k.close.to_f64().unwrap_or(0.0));
        hours.push(k.open_time.hour() as u32);
        weekdays.push(k.open_time.weekday().num_days_from_monday() as u32);
    }
    
    // --- Enhanced Technical Indicators ---
    let rsi_14 = calculate_rsi(&closes, 14);
    let rsi_14_rank = rank_normalize(Series::new("rsi_14_raw", rsi_14.clone()), 252);
    
    let macd_hist = calculate_macd_hist(&closes, 12, 26, 9);
    let macd_signal = calculate_macd_signal(&closes, 12, 26, 9);
    
    // --- Price Momentum Features ---
    let returns_1h = calculate_returns(&closes, 1);
    let returns_4h = calculate_returns(&closes, 4);
    let returns_24h = calculate_returns(&closes, 24);
    
    // --- Volatility Features ---
    let volatility_1h = calculate_volatility(&closes, 1);
    let volatility_4h = calculate_volatility(&closes, 4);
    let volatility_24h = calculate_volatility(&closes, 24);
    
    // --- Moving Averages ---
    let sma_20 = calculate_sma(&closes, 20);
    let sma_50 = calculate_sma(&closes, 50);
    let price_vs_sma20 = calculate_price_vs_ma(&closes, &sma_20);
    let price_vs_sma50 = calculate_price_vs_ma(&closes, &sma_50);
    
    // --- Bollinger Bands ---
    let bb_position = calculate_bollinger_position(&closes, 20, 2.0);
    
    // --- RSI Momentum ---
    let rsi_momentum = calculate_rsi_momentum(&rsi_14.clone());
    
    // --- Time-based Features ---
    let hour_sin = hours.iter().map(|&h| (h as f64 * std::f64::consts::PI / 12.0).sin()).collect::<Vec<f64>>();
    let hour_cos = hours.iter().map(|&h| (h as f64 * std::f64::consts::PI / 12.0).cos()).collect::<Vec<f64>>();
    let day_sin = weekdays.iter().map(|&d| (d as f64 * std::f64::consts::PI / 7.0).sin()).collect::<Vec<f64>>();
    let day_cos = weekdays.iter().map(|&d| (d as f64 * std::f64::consts::PI / 7.0).cos()).collect::<Vec<f64>>();

    // Create the enhanced Polars DataFrame (NO RAW PRICE FEATURES)
    let df = DataFrame::new(vec![
        // Technical Indicators
        Series::new("rsi_14_rank", rsi_14_rank),
        Series::new("rsi_momentum", rsi_momentum),
        Series::new("macd_hist", macd_hist),
        Series::new("macd_signal", macd_signal),
        
        // Price Momentum
        Series::new("returns_1h", returns_1h),
        Series::new("returns_4h", returns_4h),
        Series::new("returns_24h", returns_24h),
        
        // Volatility
        Series::new("volatility_1h", volatility_1h),
        Series::new("volatility_4h", volatility_4h),
        Series::new("volatility_24h", volatility_24h),
        
        // Moving Averages
        Series::new("price_vs_sma20", price_vs_sma20),
        Series::new("price_vs_sma50", price_vs_sma50),
        
        // Bollinger Bands
        Series::new("bb_position", bb_position),
        
        // Cyclical Time Features
        Series::new("hour_sin", hour_sin),
        Series::new("hour_cos", hour_cos),
        Series::new("day_sin", day_sin),
        Series::new("day_cos", day_cos),
    ])?;

    Ok(df)
}

/// Helper to calculate RSI for a series of closing prices.
fn calculate_rsi(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut rsi = Rsi::new(period).unwrap();
    closes.iter().map(|c| {
        let val = rsi.next(*c);
        if val.is_finite() { Some(val) } else { None }
    }).collect()
}

/// Helper to calculate MACD Histogram.
fn calculate_macd_hist(closes: &[f64], fast: usize, slow: usize, signal: usize) -> Vec<Option<f64>> {
    let mut macd = Macd::new(fast, slow, signal).unwrap();
    closes.iter().map(|c| {
        let val = macd.next(*c).histogram;
        if val.is_finite() { Some(val) } else { None }
    }).collect()
}

/// (The "Quirk") Converts a series to its percentile rank over a rolling window.
fn rank_normalize(series: Series, period: usize) -> Series {
    let values = series.f64().unwrap();
    let len = values.len();
    let mut ranks = Vec::with_capacity(len);
    
    for i in 0..len {
        if i < period - 1 {
            // Not enough data for the window yet
            ranks.push(None);
            continue;
        }
        
        // Get the window of values
        let window_start = i - period + 1;
        let window_values: Vec<f64> = values
            .into_no_null_iter()
            .skip(window_start)
            .take(period)
            .collect();
        
        if window_values.is_empty() {
            ranks.push(None);
            continue;
        }
        
        // Get the current value (last value in the window)
        let current_value = window_values[window_values.len() - 1];
        
        // Calculate how many values in the window are less than the current value
        let rank = window_values.iter().filter(|&&v| v < current_value).count();
        let percentile = rank as f64 / window_values.len() as f64;
        
        ranks.push(Some(percentile));
    }
    
    Series::new(series.name(), ranks)
}

/// Calculate MACD signal line
fn calculate_macd_signal(closes: &[f64], fast: usize, slow: usize, signal: usize) -> Vec<Option<f64>> {
    let mut macd = Macd::new(fast, slow, signal).unwrap();
    closes.iter().map(|c| {
        let val = macd.next(*c).signal;
        if val.is_finite() { Some(val) } else { None }
    }).collect()
}

/// Calculate returns over n periods
fn calculate_returns(closes: &[f64], periods: usize) -> Vec<Option<f64>> {
    let mut returns = Vec::with_capacity(closes.len());
    
    for i in 0..closes.len() {
        if i < periods {
            returns.push(None);
            continue;
        }
        
        let current_price = closes[i];
        let past_price = closes[i - periods];
        
        if past_price > 0.0 {
            let ret = (current_price - past_price) / past_price;
            returns.push(Some(ret));
        } else {
            returns.push(None);
        }
    }
    
    returns
}

/// Calculate volatility (standard deviation of returns)
fn calculate_volatility(closes: &[f64], window: usize) -> Vec<Option<f64>> {
    let mut volatility = Vec::with_capacity(closes.len());
    
    for i in 0..closes.len() {
        if i < window {
            volatility.push(None);
            continue;
        }
        
        let window_returns: Vec<f64> = (1..=window)
            .map(|j| {
                let current = closes[i - j + 1];
                let previous = closes[i - j];
                if previous > 0.0 { (current - previous) / previous } else { 0.0 }
            })
            .collect();
        
        let mean = window_returns.iter().sum::<f64>() / window_returns.len() as f64;
        let variance = window_returns.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / window_returns.len() as f64;
        
        volatility.push(Some(variance.sqrt()));
    }
    
    volatility
}

/// Calculate Simple Moving Average
fn calculate_sma(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut sma = Vec::with_capacity(closes.len());
    
    for i in 0..closes.len() {
        if i < period - 1 {
            sma.push(None);
            continue;
        }
        
        let sum: f64 = closes[i - period + 1..=i].iter().sum();
        sma.push(Some(sum / period as f64));
    }
    
    sma
}

/// Calculate price vs moving average ratio
fn calculate_price_vs_ma(closes: &[f64], ma: &[Option<f64>]) -> Vec<Option<f64>> {
    closes.iter().zip(ma.iter()).map(|(&price, &ma_val)| {
        ma_val.map(|ma| if ma > 0.0 { price / ma - 1.0 } else { 0.0 })
    }).collect()
}

/// Calculate Bollinger Bands position
fn calculate_bollinger_position(closes: &[f64], period: usize, std_dev: f64) -> Vec<Option<f64>> {
    let mut bb_position = Vec::with_capacity(closes.len());
    
    for i in 0..closes.len() {
        if i < period - 1 {
            bb_position.push(None);
            continue;
        }
        
        let window = &closes[i - period + 1..=i];
        let sma = window.iter().sum::<f64>() / period as f64;
        
        let variance = window.iter()
            .map(|&x| (x - sma).powi(2))
            .sum::<f64>() / period as f64;
        let std = variance.sqrt();
        
        let upper_band = sma + (std_dev * std);
        let lower_band = sma - (std_dev * std);
        
        let current_price = closes[i];
        let position = if upper_band != lower_band {
            (current_price - lower_band) / (upper_band - lower_band)
        } else {
            0.5
        };
        
        bb_position.push(Some(position));
    }
    
    bb_position
}

/// Calculate RSI momentum (change in RSI)
fn calculate_rsi_momentum(rsi_values: &[Option<f64>]) -> Vec<Option<f64>> {
    let mut momentum = Vec::with_capacity(rsi_values.len());
    
    for i in 0..rsi_values.len() {
        if i == 0 {
            momentum.push(None);
            continue;
        }
        
        match (rsi_values[i], rsi_values[i - 1]) {
            (Some(current), Some(previous)) => {
                momentum.push(Some(current - previous));
            }
            _ => momentum.push(None),
        }
    }
    
    momentum
}