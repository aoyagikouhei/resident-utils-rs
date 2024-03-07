CREATE TABLE IF NOT EXISTS public.accounts  (
  uuid UUID NOT NULL DEFAULT gen_random_uuid()
  ,content TEXT NOT NULL DEFAULT ''
  ,PRIMARY KEY(uuid)
);
