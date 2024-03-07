use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pg::PgClient;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Account {
    pub uuid: Uuid,
    pub content: String,
}

const ALL_SQL: &str = r#"
SELECT
    to_json(t1.*)
FROM
    public.accounts AS t1
WHERE
    t1.uuid <> '00000000-0000-0000-0000-000000000005'
"#;

const ONE_SQL: &str = r#"
SELECT
    to_json(t1.*)
FROM
    public.accounts AS t1
WHERE
    t1.uuid = $1
"#;

impl Account {
    pub async fn get_all(
        pg_client: &PgClient,
    ) -> Result<HashMap<Uuid, Account>, resident_utils::postgres::holder::Error> {
        let accounts: Vec<Account> = pg_client
            .query(ALL_SQL, &[])
            .await?
            .iter()
            .map(|row| {
                let json: serde_json::Value = row.get(0);
                serde_json::from_value(json).unwrap()
            })
            .collect();

        let mut map = HashMap::new();
        for account in accounts {
            map.insert(account.uuid, account);
        }
        Ok(map)
    }

    pub async fn get_one(
        pg_client: &PgClient,
        uuid: &Uuid,
    ) -> Result<Option<Account>, resident_utils::postgres::holder::Error> {
        let accounts: Vec<Account> = pg_client
            .query(ONE_SQL, &[&uuid])
            .await?
            .iter()
            .map(|row| {
                let json: serde_json::Value = row.get(0);
                serde_json::from_value(json).unwrap()
            })
            .collect();
        Ok(accounts.first().map(|a| a.clone()))
    }
}
