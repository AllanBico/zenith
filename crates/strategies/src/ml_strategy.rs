use crate::{Strategy, StrategyError};
use core_types::{Kline, OrderRequest, OrderSide, OrderType, Signal};
use ml_features::generate_features;
use polars::prelude::*;
use smartcore::ensemble::random_forest_classifier::RandomForestClassifier;
use smartcore::linalg::basic::matrix::DenseMatrix;
use std::fs::File;
use std::path::PathBuf;
use uuid::Uuid;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use ndarray::Array2;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// Reproduce the exact structures from ml-trainer to match serialization format
#[derive(Serialize, Deserialize)]
struct TrainedModel {
    feature_names: Vec<String>,
    model_type: String,
    training_info: ModelInfo,
    training_metadata: TrainingMetadata,
    preprocessing_info: PreprocessingInfo,
}

#[derive(Serialize, Deserialize)]
struct ModelInfo {
    n_samples: usize,
    n_features: usize,
    classes: Vec<usize>,
    class_distribution: HashMap<i32, usize>,
}

#[derive(Serialize, Deserialize)]
struct TrainingMetadata {
    training_date: String,
    model_parameters: ModelParameters,
    performance_metrics: PerformanceMetrics,
    cross_validation_results: Option<CrossValidationResults>,
}

#[derive(Serialize, Deserialize)]
struct ModelParameters {
    n_trees: usize,
    max_depth: Option<usize>,
    min_samples_leaf: usize,
    min_samples_split: usize,
}

#[derive(Serialize, Deserialize)]
struct PerformanceMetrics {
    accuracy: f64,
    precision: f64,
    recall: f64,
    f1_score: f64,
    confusion_matrix: Vec<Vec<usize>>,
}

#[derive(Serialize, Deserialize)]
struct CrossValidationResults {
    mean_score: f64,
    std_score: f64,
    fold_scores: Vec<f64>,
}

#[derive(Serialize, Deserialize)]
struct PreprocessingInfo {
    feature_scaling: bool,
    feature_selection: Option<Vec<usize>>,
    missing_value_strategy: String,
    scaler_means: Vec<f64>,
    scaler_stds: Vec<f64>,
}

/// Feature scaler for inference
struct FeatureScaler {
    means: Vec<f64>,
    stds: Vec<f64>,
}

impl FeatureScaler {
    fn new(means: Vec<f64>, stds: Vec<f64>) -> Self {
        Self { means, stds }
    }

    fn transform(&self, data: &Array2<f64>) -> Result<Array2<f64>, String> {
        let (n_samples, n_features) = data.dim();
        if n_features != self.means.len() {
            return Err(format!("Feature count mismatch: expected {}, got {}", self.means.len(), n_features));
        }

        let mut scaled_data = Array2::zeros((n_samples, n_features));
        for i in 0..n_samples {
            for j in 0..n_features {
                scaled_data[[i, j]] = (data[[i, j]] - self.means[j]) / self.stds[j];
            }
        }
        Ok(scaled_data)
    }
}

// This is the type of the artifact we saved in the trainer
type ModelArtifact = (
    RandomForestClassifier<f64, i32, DenseMatrix<f64>, Vec<i32>>,
    TrainedModel,
);

/// The MlStrategy uses a pre-trained model to make decisions.
pub struct MlStrategy {
    model: RandomForestClassifier<f64, i32, DenseMatrix<f64>, Vec<i32>>,
    kline_buffer: Vec<Kline>,
    min_buffer_size: usize,
    symbol: String,
    scaler: FeatureScaler,
    prediction_threshold: f64,
}

impl MlStrategy {
    /// Creates a new `MlStrategy` by loading a serialized model from disk.
    pub fn new(model_path: &PathBuf, symbol: String) -> Result<Self, StrategyError> {
        let file = File::open(model_path).map_err(|e| {
            StrategyError::InvalidParameters(format!(
                "Failed to open model file at {:?}: {}",
                model_path, e
            ))
        })?;

        // Deserialize the entire artifact
        let (model, artifact_metadata): ModelArtifact = bincode::deserialize_from(file).map_err(|e| {
            StrategyError::InvalidParameters(format!("Failed to deserialize model: {}", e))
        })?;

        tracing::info!(
            "Loaded ML model: {} features, {} samples, accuracy: {:.3}, symbol: {}",
            artifact_metadata.training_info.n_features,
            artifact_metadata.training_info.n_samples,
            artifact_metadata.training_metadata.performance_metrics.accuracy,
            symbol
        );

        // Create feature scaler from saved parameters
        let scaler = FeatureScaler::new(
            artifact_metadata.preprocessing_info.scaler_means.clone(),
            artifact_metadata.preprocessing_info.scaler_stds.clone(),
        );

        Ok(Self {
            model,
            kline_buffer: Vec::with_capacity(500), // Pre-allocate buffer
            min_buffer_size: 5, // Reduced from 252 to 60 for faster warm-up
            symbol,
            scaler,
            prediction_threshold: 0.5, // Only trade when model is confident
        })
    }
}

impl Strategy for MlStrategy {
    #[tracing::instrument(name = "ml_strategy_evaluate", skip(self, kline))]
    fn evaluate(&mut self, kline: &Kline) -> Result<Option<Signal>, StrategyError> {

        // 1. Update the historical buffer.
        self.kline_buffer.push(kline.clone());
        
        // Maintain a max buffer size to prevent memory leaks over long runs.
        if self.kline_buffer.len() > 500 {
            self.kline_buffer.remove(0);
        }

        // 2. Wait for the buffer to warm up.
        if self.kline_buffer.len() < self.min_buffer_size {
            return Ok(None); // Not enough data to generate features yet.
        }
        
        // 3. Generate features for the entire buffer.
        let features_df = generate_features(&self.kline_buffer)
            .map_err(|e| StrategyError::IndicatorError(e.to_string()))?
            .drop_nulls::<&str>(None)
            .map_err(|e| StrategyError::IndicatorError(e.to_string()))?;
        
        // We only care about the features for the most recent kline.
        let last_features = features_df.tail(Some(1));
        if last_features.height() == 0 {
            return Ok(None); // Not enough data to generate a full feature set for the last bar
        }

        // 4. Convert the last row of features into the format `smartcore` expects.
        let x_predict_ndarray: Array2<f64> = last_features.to_ndarray::<Float64Type>(IndexOrder::C)
            .map_err(|e| StrategyError::IndicatorError(e.to_string()))?;
        
        // 4.1. Apply feature scaling (CRITICAL FIX)
        let x_scaled = self.scaler.transform(&x_predict_ndarray)
            .map_err(|e| StrategyError::IndicatorError(format!("Feature scaling failed: {}", e)))?;
        
        // Convert scaled ndarray to Vec<Vec<f64>> for smartcore
        let rows = x_scaled.nrows();
        let cols = x_scaled.ncols();
        let mut data = Vec::with_capacity(rows);
        for i in 0..rows {
            let mut row = Vec::with_capacity(cols);
            for j in 0..cols {
                row.push(x_scaled[[i, j]]);
            }
            data.push(row);
        }
        
        let x_predict = DenseMatrix::from_2d_vec(&data)
            .map_err(|e| StrategyError::IndicatorError(format!("Failed to create DenseMatrix: {}", e)))?;

        // 5. Make the prediction.
        let prediction = self.model.predict(&x_predict)
            .map_err(|e| StrategyError::IndicatorError(e.to_string()))?;
            
        let prediction_value = prediction.first().unwrap_or(&0);

        // 6. Add market condition filters before generating signals
        let current_volume = kline.volume.to_f64().unwrap_or(0.0);
        let avg_volume = if self.kline_buffer.len() >= 20 {
            let recent_volumes: Vec<f64> = self.kline_buffer
                .iter()
                .rev()
                .take(20)
                .map(|k| k.volume.to_f64().unwrap_or(0.0))
                .collect();
            recent_volumes.iter().sum::<f64>() / recent_volumes.len() as f64
        } else {
            current_volume
        };

        // Only trade during higher volume periods (liquidity filter)
        if current_volume < avg_volume * 0.8 {
            tracing::debug!(
                current_volume = %current_volume,
                avg_volume = %avg_volume,
                "Skipping signal due to low volume"
            );
            return Ok(None);
        }

        // Add volatility filter - avoid extreme volatility periods
        let price_range = (kline.high - kline.low).to_f64().unwrap_or(0.0);
        let close_price = kline.close.to_f64().unwrap_or(1.0);
        let volatility_ratio = price_range / close_price;
        
        if volatility_ratio > 0.05 { // Skip if single bar moves >5%
            tracing::debug!(
                volatility_ratio = %volatility_ratio,
                "Skipping signal due to high volatility"
            );
            return Ok(None);
        }

        // 7. Generate signals based on prediction and confidence
        // We now support both BUY (Win prediction) and SELL (Loss prediction) signals
        match *prediction_value {
            1 => {
                // Win prediction - Generate BUY signal
                // Calculate dynamic confidence based on multiple factors
                let base_confidence: f64 = 0.6; // Base confidence for wins
                let volume_boost: f64 = if current_volume > avg_volume * 1.2 { 0.1 } else { 0.0 };
                let volatility_penalty: f64 = if volatility_ratio > 0.02 { -0.1 } else { 0.0 };
                
                let final_confidence = (base_confidence + volume_boost + volatility_penalty)
                    .max(0.4)
                    .min(0.8);
                    
                let confidence = Decimal::from_f64(final_confidence)
                    .unwrap_or(Decimal::from_str("0.6").unwrap());

                let signal = Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence,
                    order_request: OrderRequest {
                        client_order_id: Uuid::new_v4(),
                        symbol: self.symbol.clone(),
                        side: OrderSide::Buy,
                        order_type: OrderType::Market,
                        quantity: "1.0".parse().unwrap(), // Placeholder, risk manager will resize
                        price: None,
                        position_side: None, // Use one-way mode for now
                    },
                };
                tracing::info!(
                    confidence = %signal.confidence,
                    symbol = %self.symbol,
                    prediction = %prediction_value,
                    "ML model generated a BUY signal."
                );
                return Ok(Some(signal));
            },
            -1 => {
                // Loss prediction - Generate SELL signal
                // Apply same dynamic confidence calculation
                let base_confidence: f64 = 0.65; // Slightly higher for loss predictions
                let volume_boost: f64 = if current_volume > avg_volume * 1.2 { 0.1 } else { 0.0 };
                let volatility_penalty: f64 = if volatility_ratio > 0.02 { -0.1 } else { 0.0 };
                
                let final_confidence = (base_confidence + volume_boost + volatility_penalty)
                    .max(0.4)
                    .min(0.8);
                    
                let confidence = Decimal::from_f64(final_confidence)
                    .unwrap_or(Decimal::from_str("0.65").unwrap());

                let signal = Signal {
                    signal_id: Uuid::new_v4(),
                    timestamp: kline.close_time,
                    confidence,
                    order_request: OrderRequest {
                        client_order_id: Uuid::new_v4(),
                        symbol: self.symbol.clone(),
                        side: OrderSide::Sell,
                        order_type: OrderType::Market,
                        quantity: "1.0".parse().unwrap(), // Placeholder, risk manager will resize
                        price: None,
                        position_side: None, // Use one-way mode for now
                    },
                };
                tracing::info!(
                    confidence = %signal.confidence,
                    symbol = %self.symbol,
                    prediction = %prediction_value,
                    "ML model generated a SELL signal."
                );
                return Ok(Some(signal));
            },
            _ => {
                // Neutral prediction (0) or other - No signal
                tracing::debug!(
                    prediction = %prediction_value,
                    symbol = %self.symbol,
                    "ML model prediction neutral, no signal generated."
                );
            }
        }

        Ok(None)
    }
}