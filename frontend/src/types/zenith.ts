// Types that correspond to the JSON responses from the Zenith Rust web-server.

export interface OptimizationJob {
    job_id: string;
    strategy_id: string;
    symbol: string;
    job_status: string;
    created_at: string; // ISO 8601 date string
  }
  
  export interface RankedReport {
    parameters: Record<string, number | string>;
    score: string; // Comes as a string to preserve decimal precision
    report: FullReport;
  }
  
  export interface FullReport {
    run_id: string;
    job_id: string;
    parameters: Record<string, number | string>;
    report_id: string;
    total_net_profit: string;
    gross_profit: string;
    gross_loss: string;
    profit_factor: string | null;
    total_return_pct: string;
    max_drawdown: string;
    max_drawdown_pct: string;
    sharpe_ratio: string | null;
    calmar_ratio: string | null;
    total_trades: number;
    winning_trades: number;
    losing_trades: number;
    win_rate_pct: string | null;
    average_win: string;
    average_loss: string;
  
    payoff_ratio: string | null;
    average_holding_period: string;
    // This is a placeholder for the full trade and equity data
    trades?: Trade[];
    equity_curve?: EquityDataPoint[];
  }
  
  export interface Trade {
      trade_id: string;
      run_id: string;
      symbol: string;
      entry_price: string;
      entry_qty: string;
      entry_timestamp: string;
      exit_price: string;
      exit_qty: string;
      exit_timestamp: string;
      pnl?: string; // We can calculate this on the frontend
  }
  
  export interface EquityDataPoint {
    timestamp: string;
    equity: string;
  }
  
  // We will need a more detailed type for a full backtest run
  export interface BacktestRunDetails extends FullReport {
      trades: Trade[];
      equity_curve: EquityDataPoint[];
  }

  export interface WfoJob {
    wfo_job_id: string;
    strategy_id: string;
    symbol: string;
    in_sample_period_months: number;
    out_of_sample_period_months: number;
    wfo_status: string;
    created_at: string;
  }

  export interface WfoRun {
    wfo_run_id: string;
    wfo_job_id: string;
    oos_run_id: string;
    best_in_sample_parameters: Record<string, any>;
    oos_start_date: string;
    oos_end_date: string;
  }