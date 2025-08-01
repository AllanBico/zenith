use crate::error::ExecutorError;
use core_types::{Execution, OrderSide, Position};
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

/// Manages the state of a trading account, including cash, positions, and equity.
/// Its sole responsibility is to accurately reflect the current state based on trade executions.
#[derive(Debug, Clone)]
pub struct Portfolio {
    pub cash: Decimal,
    pub positions: HashMap<String, Position>,
}

impl Portfolio {
    /// Creates a new `Portfolio` with a given amount of starting capital.
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            cash: initial_capital,
            positions: HashMap::new(),
        }
    }

    /// Updates the portfolio state based on a trade execution.
    /// This is the core state transition logic. It does not calculate P&L, it only mutates state.
    pub fn update_with_execution(
        &mut self,
        execution: &Execution,
    ) -> Result<(), ExecutorError> {
        let cost = execution.price * execution.quantity;
        let symbol = &execution.symbol;

        // --- Cash Update ---
        // For a Buy, cash decreases. For a Sell, cash increases.
        // We also subtract the fee regardless of direction.
        match execution.side {
            OrderSide::Buy => self.cash -= cost,
            OrderSide::Sell => self.cash += cost,
        }
        self.cash -= execution.fee;

        if self.cash.is_sign_negative() {
            return Err(ExecutorError::InsufficientCash {
                required: cost.to_string(),
                available: (self.cash + cost + execution.fee).to_string(), // Add fee back for display
            });
        }

        // --- Position Update ---
        let position = self.positions.entry(symbol.clone()).or_insert_with(|| {
            // If the position does not exist, create a new one.
            Position {
                position_id: Uuid::new_v4(),
                symbol: symbol.clone(),
                side: execution.side,
                quantity: Decimal::ZERO,
                entry_price: Decimal::ZERO, // Will be calculated below
                unrealized_pnl: Decimal::ZERO, // Will be calculated by the backtester loop
                last_updated: Utc::now(),
            }
        });

        let is_closing_trade = position.quantity.is_sign_positive() && position.side != execution.side;

        if is_closing_trade {
            // Logic for closing or reducing a position.
            if execution.quantity > position.quantity {
                return Err(ExecutorError::InvalidClosingQuantity {
                    requested: execution.quantity.to_string(),
                    available: position.quantity.to_string(),
                });
            }
            position.quantity -= execution.quantity;
        } else {
            // Logic for opening or increasing a position.
            // Calculate the new average entry price.
            let existing_value = position.entry_price * position.quantity;
            let new_value = execution.price * execution.quantity;
            let total_quantity = position.quantity + execution.quantity;

            position.side = execution.side; // Ensure side is correct if opening from flat
            
            if !total_quantity.is_zero() {
                position.entry_price = (existing_value + new_value) / total_quantity;
            }
            position.quantity += execution.quantity;
        }

        position.last_updated = execution.timestamp;

        // If position quantity is zero after an update, remove it from the map.
        if position.quantity.is_zero() {
            self.positions.remove(symbol);
        }

        Ok(())
    }

    /// Calculates the total equity of the portfolio at a given set of market prices.
    /// Equity = Cash + Market Value of all open positions.
    pub fn calculate_total_equity(
        &self,
        market_prices: &HashMap<String, Decimal>,
    ) -> Result<Decimal, ExecutorError> {
        let mut positions_value = Decimal::ZERO;

        for (symbol, position) in &self.positions {
            let current_price = market_prices.get(symbol).ok_or_else(|| {
                ExecutorError::PortfolioError(format!("Missing market price for symbol: {}", symbol))
            })?;
            
            // For long positions, value is price * qty. For short, it's more complex,
            // but for equity calculation, we care about the value of closing it.
            // A simpler way is (entry_value + unrealized_pnl).
            let pnl_per_unit = match position.side {
                OrderSide::Buy => *current_price - position.entry_price,
                OrderSide::Sell => position.entry_price - *current_price,
            };
            
            let market_value = (position.entry_price * position.quantity) + (pnl_per_unit * position.quantity);
            positions_value += market_value;
        }

        Ok(self.cash + positions_value)
    }

    /// A simple utility to get a snapshot of a single position.
    pub fn get_position(&self, symbol: &str) -> Option<&Position> {
        self.positions.get(symbol)
    }
}