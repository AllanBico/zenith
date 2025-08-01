use crate::error::ExecutorError;
use core_types::{Execution, OrderSide, Position};
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;

/// Manages the state of a trading account, including cash, positions, and equity.
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
    /// This is the core logic that handles opening, closing, and modifying positions.
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
                available: (self.cash + cost).to_string(),
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
                entry_price: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                last_updated: Utc::now(),
            }
        });
        
        let is_closing_trade = position.side != execution.side;

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
            
            if !total_quantity.is_zero() {
                position.entry_price = (existing_value + new_value) / total_quantity;
            }
            position.quantity += execution.quantity;
        }

        position.last_updated = Utc::now();
        
        // If position quantity is zero after an update, remove it.
        if position.quantity.is_zero() {
            self.positions.remove(symbol);
        }

        Ok(())
    }

    /// Calculates the total equity of the portfolio at a given set of market prices.
    /// Equity = Cash + Value of all open positions.
    pub fn calculate_total_equity(
        &self,
        market_prices: &HashMap<String, Decimal>,
    ) -> Result<Decimal, ExecutorError> {
        let mut positions_value = Decimal::ZERO;

        for (symbol, position) in &self.positions {
            let current_price = market_prices.get(symbol).ok_or_else(|| {
                ExecutorError::PortfolioError(format!("Missing market price for symbol: {}", symbol))
            })?;
            
            let pnl_per_unit = *current_price - position.entry_price;
            let position_pnl = pnl_per_unit * position.quantity;
            
            let market_value = (position.entry_price * position.quantity) + position_pnl;
            positions_value += market_value;
        }

        Ok(self.cash + positions_value)
    }
}