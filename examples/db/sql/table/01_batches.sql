CREATE TABLE IF NOT EXISTS public.batches  (
  batch_code TEXT NOT NULL
  ,batch_start_at TIMESTAMPTZ
  ,batch_enable_second_count BIGINT NOT NULL DEFAULT 0
  ,PRIMARY KEY(batch_code)
);
