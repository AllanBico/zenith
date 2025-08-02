use crate::error::OptimizerError;
use configuration::optimizer_config::{OptimizerConfig, ParameterRange};
use itertools::Itertools;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// Generates every unique combination of parameters from the defined parameter space.
pub fn generate_parameter_sets(
    config: &OptimizerConfig,
) -> Result<Vec<Value>, OptimizerError> {
    let mut parameter_values: HashMap<String, Vec<Value>> = HashMap::new();

    // 1. Convert all parameter ranges into concrete lists of values.
    for (name, range) in &config.parameter_space {
        let values = match range {
            ParameterRange::DiscreteInt(vals) => vals.iter().map(|&v| json!(v)).collect(),
            ParameterRange::DiscreteDecimal(vals) => vals.iter().map(|v| json!(v)).collect(),
            ParameterRange::LinearInt { start, end, step } => {
                if *step <= 0 {
                    return Err(OptimizerError::ParameterGeneration(format!(
                        "Step for '{}' must be positive.",
                        name
                    )));
                }
                (*start..=*end).step_by(*step as usize).map(|v| json!(v)).collect()
            }
            ParameterRange::LinearDecimal { start, end, step } => {
                if step.is_sign_negative() || step.is_zero() {
                     return Err(OptimizerError::ParameterGeneration(format!(
                        "Step for '{}' must be positive.",
                        name
                    )));
                }
                let mut vals = Vec::new();
                let mut current = *start;
                while current <= *end {
                    vals.push(json!(current));
                    current += *step;
                }
                vals
            }
        };
        parameter_values.insert(name.clone(), values);
    }

    // 2. Use itertools::multi_cartesian_product to generate all combinations.
    let (param_names, value_lists): (Vec<_>, Vec<_>) = parameter_values.into_iter().unzip();
    
    let combinations = value_lists
        .into_iter()
        .multi_cartesian_product()
        .map(|product| {
            let mut map = Map::new();
            for (i, value) in product.into_iter().enumerate() {
                map.insert(param_names[i].clone(), value);
            }
            Value::Object(map)
        })
        .collect();

    Ok(combinations)
}