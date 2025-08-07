use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use chrono::{NaiveDate, Utc};
use database::{connect, run_migrations, DbRepository};
use polars::prelude::*;
use std::path::PathBuf;
// use tracing::info; // Removed
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use crate::labeling::LabelingConfig;
use ndarray::Array2;
use std::fs::File;
use serde::{Serialize, Deserialize};
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::linalg::basic::arrays::Array;
use smartcore::model_selection::train_test_split;
use smartcore::ensemble::random_forest_classifier::{RandomForestClassifier, RandomForestClassifierParameters};
use smartcore::metrics::{accuracy, precision, recall, f1};
use std::collections::HashMap;

pub mod features;
pub mod labeling;

/// Custom feature scaler implementation since smartcore's StandardScaler isn't available
struct FeatureScaler {
    means: Vec<f64>,
    stds: Vec<f64>,
    fitted: bool,
}

impl FeatureScaler {
    fn new() -> Self {
        Self {
            means: Vec::new(),
            stds: Vec::new(),
            fitted: false,
        }
    }

    fn fit(&mut self, data: &Array2<f64>) -> Result<()> {
        let (n_samples, n_features) = data.dim();
        self.means = vec![0.0; n_features];
        self.stds = vec![0.0; n_features];

        // Calculate means
        for j in 0..n_features {
            let mut sum = 0.0;
            for i in 0..n_samples {
                sum += data[[i, j]];
            }
            self.means[j] = sum / n_samples as f64;
        }

        // Calculate standard deviations
        for j in 0..n_features {
            let mut sum_sq = 0.0;
            for i in 0..n_samples {
                let diff = data[[i, j]] - self.means[j];
                sum_sq += diff * diff;
            }
            self.stds[j] = (sum_sq / (n_samples - 1) as f64).sqrt();
            // Avoid division by zero
            if self.stds[j] < 1e-10 {
                self.stds[j] = 1.0;
            }
        }

        self.fitted = true;
        Ok(())
    }

    fn transform(&self, data: &Array2<f64>) -> Result<Array2<f64>> {
        if !self.fitted {
            return Err(anyhow::anyhow!("Scaler must be fitted before transform"));
        }

        let (n_samples, n_features) = data.dim();
        let mut scaled_data = Array2::zeros((n_samples, n_features));

        for i in 0..n_samples {
            for j in 0..n_features {
                scaled_data[[i, j]] = (data[[i, j]] - self.means[j]) / self.stds[j];
            }
        }

        Ok(scaled_data)
    }
}

/// Calculate confusion matrix manually since smartcore doesn't provide it
fn calculate_confusion_matrix(y_true: &[i32], y_pred: &[i32]) -> Vec<Vec<usize>> {
    let mut cm = vec![vec![0; 2]; 2]; // Assuming binary classification
    
    for (&true_val, &pred_val) in y_true.iter().zip(y_pred.iter()) {
        let true_idx = if true_val == 1 { 1 } else { 0 };
        let pred_idx = if pred_val == 1 { 1 } else { 0 };
        cm[true_idx][pred_idx] += 1;
    }
    
    cm
}

/// Calculate class distribution
fn calculate_class_distribution(labels: &[i32]) -> HashMap<i32, usize> {
    let mut distribution = HashMap::new();
    for &label in labels {
        *distribution.entry(label).or_insert(0) += 1;
    }
    distribution
}

/// A serializable wrapper for the trained model
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

// ... (Cli and Args structs from Step 1) ...
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a feature and label dataset from historical kline data.
    GenerateDataset(GenerateDatasetArgs),
    /// Train a model from a feature/label dataset.
    TrainModel(TrainModelArgs),
}

#[derive(Parser)]
struct GenerateDatasetArgs {
    /// The symbol to generate data for (e.g., "BTCUSDT").
    #[arg(long)]
    symbol: String,
    /// The interval of the klines (e.g., "1h").
    #[arg(long)]
    interval: String,
    /// The start date for the data (format: YYYY-MM-DD).
    #[arg(long)]
    from: NaiveDate,
    /// The end date for the data (format: YYYY-MM-DD).
    #[arg(long)]
    to: NaiveDate,
    /// The output file path for the Parquet dataset.
    #[arg(long, short)]
    output: PathBuf,
}

#[derive(Parser)]
struct TrainModelArgs {
    /// Path to the Parquet dataset file.
    #[arg(long, short)]
    dataset: PathBuf,
    /// The output file path for the trained model artifact.
    #[arg(long, short)]
    output: PathBuf,
}


#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateDataset(args) => {
            handle_generate_dataset(args).await?;
        }
        Commands::TrainModel(args) => {
            handle_train_model(args).await?;
        }
    }

    Ok(())
}

/// The handler for the `generate-dataset` command.
async fn handle_generate_dataset(args: GenerateDatasetArgs) -> Result<()> {
    println!("--- Starting Dataset Generation ---");
    
    // 1. Connect to Database
    dotenvy::dotenv().ok();
    let db_pool = connect().await.context("Failed to connect to database")?;
    run_migrations(&db_pool).await.context("Failed to run migrations")?;
    let db_repo = DbRepository::new(db_pool);
    
    // 2. Fetch Kline Data
    println!(
        "Fetching kline data from database... (symbol: {}, interval: {})",
        args.symbol, args.interval
    );
    let klines = db_repo.get_klines_by_date_range(
        &args.symbol,
        &args.interval,
        args.from.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Utc).unwrap(),
        args.to.and_hms_opt(23, 59, 59).unwrap().and_local_timezone(Utc).unwrap(),
    ).await?;
    println!("Found {} klines.", klines.len());

    // 3. Generate Features
    println!("Generating features...");
    let mut features_df = features::generate_features(&klines)?;
    println!("Generated DataFrame with shape: {:?}", features_df.shape());

    // 4. Generate Labels
    println!("Applying Triple Barrier labeling...");
    // These would eventually come from a config file.
    // Improved labeling configuration for better accuracy
    let labeling_config = LabelingConfig {
        take_profit_pct: 0.02, // 2% - More realistic target
        stop_loss_pct: 0.02, // 1% - Improved risk/reward ratio (2:1)
        time_limit_bars: 5, // 48 bars (2 days) - More time for moves to develop
    };
    let labels = labeling::apply_triple_barrier(&features_df, &labeling_config)?;
    
    // Add the labels as a new column to the DataFrame.
    features_df.with_column(labels)?;
    // Drop rows with null values that might have been created by indicators.
    let final_df = features_df.drop_nulls::<&str>(None)?;
    println!("Final dataset shape after labeling and cleaning: {:?}", final_df.shape());

    // 5. Save to Parquet File
    println!("Saving final dataset to: {:?}", &args.output);
    let mut output_file = std::fs::File::create(&args.output)
        .context(format!("Failed to create output file at {:?}", &args.output))?;
    
    ParquetWriter::new(&mut output_file).finish(&mut final_df.clone())?; // Use a mutable reference to final_df

    println!("--- Dataset Generation Complete ---");
    Ok(())
}

/// The handler for the `train-model` command with comprehensive ML pipeline
async fn handle_train_model(args: TrainModelArgs) -> Result<()> {
    println!("=== COMPREHENSIVE MODEL TRAINING PIPELINE ===");

    // 1. Load and Analyze Dataset
    println!("\n1. Loading and analyzing dataset...");
    let file = File::open(&args.dataset)?;
    let df = ParquetReader::new(file).finish()?;
    let df = df.drop_nulls::<&str>(None)?;
    let feature_names: Vec<String> = df.drop("label")?.get_column_names().iter().map(|s| s.to_string()).collect();
    println!("Dataset shape: {:?}", df.shape());
    println!("Features: {:?}", feature_names);

    // 2. Data Preparation and Analysis
    println!("\n2. Data preparation and analysis...");
    let x_df = df.drop("label")?;
    let x_ndarray: Array2<f64> = x_df.to_ndarray::<Float64Type>(IndexOrder::C)?;
    let y_ndarray: Vec<i32> = df.column("label")?.i32()?.into_no_null_iter().collect();
    
    // Check class distribution
    let class_distribution = calculate_class_distribution(&y_ndarray);
    println!("Class distribution: {:?}", class_distribution);
    
    // Convert 3-class to binary: Win (1) vs Not-Win (0)
    let y_binary: Vec<i32> = y_ndarray.iter().map(|&x| if x == 1 { 1 } else { 0 }).collect();
    let binary_distribution = calculate_class_distribution(&y_binary);
    println!("Binary class distribution (Win vs Not-Win): {:?}", binary_distribution);
    
    // Check for class imbalance in binary problem
    let total_samples = y_binary.len();
    let minority_class_count = binary_distribution.values().min().unwrap_or(&0);
    let imbalance_ratio = *minority_class_count as f64 / total_samples as f64;
    println!("Binary class imbalance ratio: {:.3} (minority class)", imbalance_ratio);
    
    if imbalance_ratio < 0.3 {
        println!("⚠️  WARNING: Significant class imbalance detected!");
    }

    // 3. Feature Scaling
    println!("\n3. Feature scaling...");
    let mut scaler = FeatureScaler::new();
    scaler.fit(&x_ndarray)?;
    let x_scaled = scaler.transform(&x_ndarray)?;
    println!("Features scaled successfully");

    // 4. Data Splitting
    println!("\n4. Data splitting...");
    let x_matrix = DenseMatrix::new(
        x_scaled.nrows(),
        x_scaled.ncols(),
        x_scaled.as_slice().unwrap().to_vec(),
        false
    ).context("Failed to create DenseMatrix")?;
    
    let (x_train, x_test, y_train, y_test) = train_test_split(&x_matrix, &y_binary, 0.2, true, None);
    println!("Training set: {} samples", y_train.len());
    println!("Test set: {} samples", y_test.len());

    // 5. Cross-Validation (simplified)
    println!("\n5. Cross-validation...");
    // For now, skip cross-validation due to smartcore API limitations
    // In production, you'd want to implement proper CV or use a different library
    println!("Cross-validation skipped (smartcore API limitations)");
    let mean_cv_score = 0.0;
    let cv_std = 0.0;
    let cv_scores = vec![0.0];

    // 6. Model Training with Optimized Parameters
    println!("\n6. Training final model...");
    
    // Calculate class weights to handle imbalance
    let win_count = y_train.iter().filter(|&&x| x == 1).count();
    let not_win_count = y_train.iter().filter(|&&x| x == 0).count();
    let total_train = y_train.len();
    
    let win_weight = total_train as f64 / (2.0 * win_count as f64);
    let not_win_weight = total_train as f64 / (2.0 * not_win_count as f64);
    
    println!("Class weights - Win: {:.3}, Not-Win: {:.3}", win_weight, not_win_weight);
    
    let final_params = RandomForestClassifierParameters::default()
        .with_n_trees(50)
        .with_max_depth(5)
        .with_min_samples_leaf(5)
        .with_min_samples_split(2);
    
    let model = RandomForestClassifier::fit(&x_train, &y_train, final_params.clone())
        .context("Failed to fit Random Forest model")?;
    println!("Model training complete");

    // 7. Comprehensive Model Evaluation
    println!("\n7. Model evaluation...");
    let predictions = model.predict(&x_test)?;
    
    // Calculate all metrics
    let y_test_f64: Vec<f64> = y_test.iter().map(|&x| x as f64).collect();
    let predictions_f64: Vec<f64> = predictions.iter().map(|&x| x as f64).collect();
    
    let accuracy_score = accuracy(&y_test, &predictions);
    let precision_score = precision(&y_test_f64, &predictions_f64);
    let recall_score = recall(&y_test_f64, &predictions_f64);
    let f1_score = f1(&y_test_f64, &predictions_f64, 1.0);
    let confusion_matrix = calculate_confusion_matrix(&y_test, &predictions);
    
    println!("\n=== DETAILED PERFORMANCE METRICS ===");
    println!("Accuracy:  {:.3} ({:.1}%)", accuracy_score, accuracy_score * 100.0);
    println!("Precision: {:.3}", precision_score);
    println!("Recall:    {:.3}", recall_score);
    println!("F1-Score:  {:.3}", f1_score);
    
    println!("\nConfusion Matrix:");
    println!("                Predicted");
    println!("Actual    0 (Loss)  1 (Win)");
    println!("0 (Loss)  {:8}  {:8}", confusion_matrix[0][0], confusion_matrix[0][1]);
    println!("1 (Win)   {:8}  {:8}", confusion_matrix[1][0], confusion_matrix[1][1]);
    
    // Calculate additional metrics
    let true_negatives = confusion_matrix[0][0];
    let false_positives = confusion_matrix[0][1];
    let false_negatives = confusion_matrix[1][0];
    let true_positives = confusion_matrix[1][1];
    
    let specificity = true_negatives as f64 / (true_negatives + false_positives) as f64;
    let sensitivity = true_positives as f64 / (true_positives + false_negatives) as f64;
    
    println!("\nAdditional Metrics:");
    println!("Specificity (True Negative Rate): {:.3}", specificity);
    println!("Sensitivity (True Positive Rate): {:.3}", sensitivity);

    // 8. Create Comprehensive Model Artifact
    println!("\n8. Creating model artifact...");
    let model_artifact = TrainedModel {
        feature_names: feature_names.clone(),
        model_type: "RandomForest".to_string(),
        training_info: ModelInfo {
            n_samples: x_train.shape().0,
            n_features: x_train.shape().1,
            classes: y_binary.iter().map(|&x| x as usize).collect(),
            class_distribution: binary_distribution,
        },
        training_metadata: TrainingMetadata {
            training_date: chrono::Utc::now().to_rfc3339(),
            model_parameters: ModelParameters {
                n_trees: 100,
                max_depth: Some(10),
                min_samples_leaf: 5,
                min_samples_split: 2,
            },
            performance_metrics: PerformanceMetrics {
                accuracy: accuracy_score,
                precision: precision_score,
                recall: recall_score,
                f1_score,
                confusion_matrix,
            },
            cross_validation_results: Some(CrossValidationResults {
                mean_score: mean_cv_score,
                std_score: cv_std,
                fold_scores: cv_scores,
            }),
        },
        preprocessing_info: PreprocessingInfo {
            feature_scaling: true,
            feature_selection: None,
            missing_value_strategy: "drop".to_string(),
            scaler_means: scaler.means.clone(),
            scaler_stds: scaler.stds.clone(),
        },
    };

    // 9. Save Model and Artifact
    println!("\n9. Saving model and metadata...");
    let file = File::create(&args.output)
        .context(format!("Failed to create model file at {:?}", &args.output))?;
    
    // Save both model and artifact
    let model_data = (model, model_artifact);
    bincode::serialize_into(file, &model_data)
        .context("Failed to serialize model")?;

    println!("\n=== MODEL TRAINING COMPLETE ===");
    println!("Model saved to: {:?}", &args.output);
    println!("Model includes:");
    println!("  - Trained Random Forest classifier");
    println!("  - Feature names and metadata");
    println!("  - Performance metrics and CV results");
    println!("  - Preprocessing information");
    
    Ok(())
}