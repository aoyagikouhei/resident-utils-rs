DROP TYPE IF EXISTS type_resident_set_delete_worker CASCADE;
CREATE TYPE type_resident_set_delete_worker AS (
  data_json JSONB
);

-- 処理を削除する
-- 引数
-- 戻り値
--   data_json : データJSON
CREATE OR REPLACE FUNCTION resident_set_delete_worker(
) RETURNS SETOF type_resident_set_delete_worker AS $FUNCTION$
DECLARE
  w_record RECORD;
  w_count BIGINT := 0;
BEGIN
  LOOP
    w_count := w_count + 1;

    -- 処理対象を取得
    SELECT
      t1.uuid
      ,t1.data_json
    INTO
      w_record
    FROM
      public.workers AS t1
    LIMIT
      1
    ;

    -- 存在しなければエラー
    IF w_record.uuid IS NULL THEN
      RETURN;
    END IF;

    DELETE FROM public.workers
    WHERE
      uuid = w_record.uuid
    ;

    -- 更新チェック
    IF FOUND THEN
      RETURN QUERY SELECT
        w_record.data_json
      ;
      RETURN;
    END IF;

    -- ループストッパー
    IF w_count >= 10 THEN
      RETURN;
    END IF;
  END LOOP;
END;
$FUNCTION$ LANGUAGE plpgsql;