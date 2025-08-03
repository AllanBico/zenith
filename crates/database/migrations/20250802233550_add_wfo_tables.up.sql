-- Add up migration script here
-- Add WFO (Walk-Forward Optimization) Tables
-- These tables store the results of our most rigorous strategy validation method.

-- A top-level record for a single Walk-Forward Optimization experiment.
CREATE TABLE wfo_jobs (
    wfo_job_id UUID PRIMARY KEY,
    strategy_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    in_sample_period_months INTEGER NOT NULL,
    out_of_sample_period_months INTEGER NOT NULL,
    wfo_status TEXT NOT NULL, -- e.g., 'Running', 'Completed', 'Failed'
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- A record for a single out-of-sample (OOS) validation run within a WFO job.
-- This table is crucial, as the stitched-together results of these runs
-- form the final, most realistic performance report for the strategy.
CREATE TABLE wfo_runs (
    wfo_run_id UUID PRIMARY KEY,
    wfo_job_id UUID NOT NULL REFERENCES wfo_jobs(wfo_job_id) ON DELETE CASCADE,
    
    -- The specific `backtest_run` record that holds the full results
    -- (report, trades, equity curve) for this OOS period.
    oos_run_id UUID NOT NULL UNIQUE REFERENCES backtest_runs(run_id) ON DELETE CASCADE,

    -- The parameters that were found to be "best" during the preceding
    -- in-sample optimization period.
    best_in_sample_parameters JSONB NOT NULL,
    
    -- The date range for this specific out-of-sample test.
    oos_start_date TIMESTAMPTZ NOT NULL,
    oos_end_date TIMESTAMPTZ NOT NULL
);

-- Add an index for quickly retrieving all runs for a given WFO job.
CREATE INDEX idx_wfo_runs_wfo_job_id ON wfo_runs (wfo_job_id);