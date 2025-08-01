-- Add the close_time column to the klines table
ALTER TABLE klines
ADD COLUMN close_time TIMESTAMPTZ NOT NULL DEFAULT NOW();

-- Create an index on close_time for better query performance
CREATE INDEX idx_klines_close_time ON klines(close_time);
