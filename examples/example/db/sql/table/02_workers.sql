CREATE TABLE IF NOT EXISTS public.workers  (
  uuid UUID NOT NULL DEFAULT gen_random_uuid()
  ,data_json JSONB NOT NULL DEFAULT '{}'
  ,PRIMARY KEY(uuid)
);
