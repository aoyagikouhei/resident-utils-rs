DELETE FROM batches;
DELETE FROM workers;
DELETE FROM accounts;

INSERT INTO batches(
    batch_code
    ,batch_enable_second_count
) VALUES (
    'minutely_batch'
    ,5
);

INSERT INTO accounts(
    uuid
    ,content
) VALUES 
('00000000-0000-0000-0000-000000000001','1'), 
('00000000-0000-0000-0000-000000000002','2'), 
('00000000-0000-0000-0000-000000000003','3'), 
('00000000-0000-0000-0000-000000000004','4'), 
('00000000-0000-0000-0000-000000000005','5');