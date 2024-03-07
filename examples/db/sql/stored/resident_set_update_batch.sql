DROP TYPE IF EXISTS type_resident_set_update_batch CASCADE;
CREATE TYPE type_resident_set_update_batch AS (
  batch_code TEXT
);

-- バッチを開始する
-- 引数
--   p_batch_code : バッチコード
--   p_now : 現在時刻
-- 戻り値
--   batch_code : バッチUUID
CREATE OR REPLACE FUNCTION resident_set_update_batch(
  p_batch_code TEXT DEFAULT NULL
  ,p_now TIMESTAMP DEFAULT NULL
) RETURNS SETOF type_resident_set_update_batch AS $FUNCTION$
DECLARE
  w_now TIMESTAMP := COALESCE(p_now, NOW());
  w_batch RECORD;
BEGIN
  -- パラメーターチェック
  IF p_batch_code IS NULL OR '' = p_batch_code THEN
    RAISE SQLSTATE 'U0002' USING MESSAGE = 'p_batch_code is null';
  END IF;

  -- 有効なバッチレコードを取得する
  SELECT
    t1.batch_code
    ,t1.batch_start_at
    ,t1.batch_enable_second_count
  INTO
    w_batch
  FROM
    public.batches AS t1
  WHERE
    t1.batch_code = p_batch_code
  LIMIT
    1
  ;

  -- 存在しなければエラー
  IF w_batch.batch_code IS NULL THEN
    RAISE SQLSTATE 'U0003' USING MESSAGE = 'p_batch_code not found ' || p_batch_code;
  END IF;

  -- 開始チェック
  IF w_batch.batch_start_at IS NOT NULL 
    AND w_batch.batch_start_at >= w_now - (w_batch.batch_enable_second_count || 'second')::interval
  THEN 
    RETURN;
  END IF;

  -- バッチの更新
  UPDATE batches SET
    batch_start_at = w_now
  WHERE
    batch_code = w_batch.batch_code
    AND (
      batch_start_at IS NULL
      OR batch_start_at = w_batch.batch_start_at
    )
  ;

  -- 更新チェック
  IF NOT FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY SELECT
    w_batch.batch_code
  ;
END;
$FUNCTION$ LANGUAGE plpgsql;