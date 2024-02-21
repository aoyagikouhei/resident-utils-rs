DELETE FROM batches;
DELETE FROM workers;


INSERT INTO batches(
    batch_code
    ,batch_enable_second_count
) VALUES (
    'minutely_batch'
    ,5
);