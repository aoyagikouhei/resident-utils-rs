use std::{collections::HashMap, future::Future, hash::Hash, time::Duration};

use chrono::prelude::*;

use thiserror::Error;

use super::SqlxPool;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid {0}")]
    Invalid(String),
}

pub struct HolderMap<K, V> {
    map: HashMap<K, V>,
    expire_interval: Duration,
    expire_at: DateTime<Utc>,
    pg_pool: SqlxPool,
}

impl<K, V> HolderMap<K, V>
where
    K: PartialEq + Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(pg_pool: SqlxPool, expire_interval: Duration, now: Option<DateTime<Utc>>) -> Self {
        Self {
            map: HashMap::new(),
            expire_interval,
            expire_at: now.unwrap_or(Utc::now()),
            pg_pool,
        }
    }

    pub async fn get<FutOne, FutAll>(
        &mut self,
        key: &K,
        now: Option<DateTime<Utc>>,
        f: impl FnOnce(SqlxPool, K) -> FutOne,
        g: impl FnOnce(SqlxPool) -> FutAll,
    ) -> Result<Option<V>, Error>
    where
        FutOne: Future<Output = Result<Option<V>, Error>>,
        FutAll: Future<Output = Result<HashMap<K, V>, Error>>,
    {
        if get_now(now) >= self.expire_at {
            let pg_client = self.pg_pool.clone();
            self.map = g(pg_client).await?;
            self.expire_at = expire_at(now, self.expire_interval);
        }
        if let Some(value) = self.map.get(key) {
            return Ok(Some(value.clone()));
        }
        let pg_client = self.pg_pool.clone();
        let Some(value) = f(pg_client, key.clone()).await? else {
            return Ok(None);
        };
        self.map.insert(key.clone(), value.clone());
        Ok(Some(value))
    }
}

pub struct HolderMapEachExpire<K, V> {
    map: HashMap<K, (V, DateTime<Utc>)>,
    expire_interval: Duration,
    pg_pool: SqlxPool,
}

impl<K, V> HolderMapEachExpire<K, V>
where
    K: PartialEq + Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(pg_pool: SqlxPool, expire_interval: Duration) -> Self {
        Self {
            map: HashMap::new(),
            expire_interval,
            pg_pool,
        }
    }

    pub async fn get<FutOne>(
        &mut self,
        key: &K,
        now: Option<DateTime<Utc>>,
        f: impl FnOnce(SqlxPool, K) -> FutOne,
    ) -> Result<Option<V>, Error>
    where
        FutOne: Future<Output = Result<Option<V>, Error>>,
    {
        match self.map.get(key) {
            Some((value, expire_at)) if get_now(now) < *expire_at => {
                return Ok(Some(value.clone()));
            }
            _ => {}
        }
        let pg_client = self.pg_pool.clone();
        let Some(value) = f(pg_client, key.clone()).await? else {
            return Ok(None);
        };
        self.map.insert(
            key.clone(),
            (value.clone(), expire_at(now, self.expire_interval)),
        );
        Ok(Some(value))
    }
}

fn get_now(now: Option<DateTime<Utc>>) -> DateTime<Utc> {
    now.unwrap_or(Utc::now())
}

fn expire_at(now: Option<DateTime<Utc>>, interval: Duration) -> DateTime<Utc> {
    get_now(now) + interval
}
