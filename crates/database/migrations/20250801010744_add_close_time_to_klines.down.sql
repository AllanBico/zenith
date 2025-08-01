-- Remove the index on close_time
DROP INDEX IF EXISTS idx_klines_close_time;

-- Remove the close_time column from the klines table
ALTER TABLE klines
DROP COLUMN IF EXISTS close_time;
