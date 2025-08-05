# Zenith Optimizer - Universal Strategy Optimization

The Zenith optimizer now supports **ALL available strategies** with a single configuration file. This guide shows you how to optimize each strategy.

## Available Strategies

1. **MACrossover** - Moving Average Crossover strategy
2. **SuperTrend** - SuperTrend with ADX filter
3. **ProbReversion** - Probabilistic Mean Reversion
4. **FundingRateArb** - Funding Rate Arbitrage

## How to Use

### 1. Choose Your Strategy

Edit `optimizer.toml` and set the `strategy_id` in the `[base_config]` section:

```toml
[base_config]
strategy_id = "MACrossover"  # or "SuperTrend", "ProbReversion", "FundingRateArb"
symbol = "BTCUSDT"
interval = "1m"
```

### 2. Configure Parameter Space

Uncomment the appropriate `[parameter_space]` section for your chosen strategy:

#### MACrossover Strategy
```toml
[parameter_space]
ma_fast_period = { start = 1, end = 20, step = 1 }
ma_slow_period = { start = 21, end = 100, step = 3 }
trend_filter_period = { start = 50, end = 200, step = 10 }
```

#### SuperTrend Strategy
```toml
[parameter_space]
atr_period = { start = 10, end = 30, step = 2 }
atr_multiplier = { start = 2.0, end = 5.0, step = 0.5 }
adx_threshold = { start = 20, end = 40, step = 5 }
adx_period = { start = 10, end = 20, step = 2 }
```

#### ProbReversion Strategy
```toml
[parameter_space]
bb_period = { start = 15, end = 30, step = 5 }
bb_std_dev = { start = 1.5, end = 3.0, step = 0.5 }
rsi_period = { start = 10, end = 20, step = 2 }
rsi_oversold = { start = 20, end = 35, step = 5 }
rsi_overbought = { start = 65, end = 80, step = 5 }
adx_threshold = { start = 15, end = 25, step = 5 }
adx_period = { start = 10, end = 20, step = 2 }
```

#### FundingRateArb Strategy
```toml
[parameter_space]
target_rate_threshold = { start = 0.0005, end = 0.002, step = 0.0005 }
basis_safety_threshold = { start = 0.003, end = 0.008, step = 0.001 }
```

### 3. Run the Optimizer

```bash
cargo run -- optimize
```

## Parameter Space Configuration

### Integer Parameters
```toml
parameter_name = { start = 10, end = 50, step = 5 }
```

### Decimal Parameters
```toml
parameter_name = { start = 1.5, end = 3.0, step = 0.5 }
```

### Discrete Values
```toml
parameter_name = [10, 20, 30, 40, 50]
```

## Analysis Configuration

The optimizer uses the same analysis configuration for all strategies:

```toml
[analysis]
[analysis.filters]
min_total_trades = 2
max_drawdown_pct = 70.0

[analysis.scoring_weights]
weight_profit_factor = 0.3
weight_calmar_ratio = 0.5
weight_avg_win_loss_ratio = 0.2
```

## Walk-Forward Optimization (WFO)

Enable WFO by uncommenting the `[wfo]` section:

```toml
[wfo]
in_sample_weeks = 8
out_of_sample_weeks = 2
```

## Example: Optimizing SuperTrend Strategy

1. Set `strategy_id = "SuperTrend"` in `[base_config]`
2. Comment out the MACrossover `[parameter_space]` section
3. Uncomment the SuperTrend `[parameter_space]` section
4. Run `cargo run -- optimize`

## Results

After optimization completes:
1. Run `cargo run -- analyze <job_id>` to see results
2. View results in the React UI at `http://localhost:3000`
3. Export results for further analysis

## Tips

- **Start Small**: Begin with narrow parameter ranges to test the setup
- **Monitor Progress**: The optimizer shows real-time progress with completion estimates
- **Parallel Processing**: The optimizer uses all available CPU cores
- **Database Storage**: All results are stored in the database for later analysis
- **Risk Management**: All optimizations use the same risk management settings from `config.toml`

## Troubleshooting

- **No Trades**: Increase `min_total_trades` or adjust parameter ranges
- **High Drawdown**: Lower `max_drawdown_pct` or adjust risk management
- **Slow Performance**: Reduce parameter space size or use WFO for faster results 