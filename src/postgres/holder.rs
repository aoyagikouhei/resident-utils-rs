use std::{collections::HashMap, future::Future, hash::Hash, time::Duration};

use chrono::prelude::*;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("PostgresPool {0}")]
    PostgresPool(#[from] deadpool_postgres::PoolError),

    #[error("Postgres {0}")]
    Postgres(#[from] deadpool_postgres::tokio_postgres::Error),
}

pub struct HolderMap<K, V> {
    map: HashMap<K, V>,
    expire_interval: Duration,
    expire_at: DateTime<Utc>,
    pg_pool: deadpool_postgres::Pool,
}

impl<K, V> HolderMap<K, V>
where
    K: PartialEq + Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(
        pg_pool: deadpool_postgres::Pool,
        expire_interval: Duration,
        now: Option<DateTime<Utc>>,
    ) -> Self {
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
        f: impl FnOnce(deadpool_postgres::Client, K) -> FutOne,
        g: impl FnOnce(deadpool_postgres::Client) -> FutAll,
    ) -> Result<Option<V>, Error>
    where
        FutOne: Future<Output = Result<Option<V>, Error>>,
        FutAll: Future<Output = Result<HashMap<K, V>, Error>>,
    {
        let now = now.unwrap_or(Utc::now());
        if now >= self.expire_at {
            let pg_client = self.pg_pool.get().await?;
            self.map = g(pg_client).await?;
            self.expire_at = now + self.expire_interval;
        }
        if let Some(value) = self.map.get(key) {
            return Ok(Some(value.clone()));
        }
        let pg_client = self.pg_pool.get().await?;
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
    pg_pool: deadpool_postgres::Pool,
}

impl<K, V> HolderMapEachExpire<K, V>
where
    K: PartialEq + Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(pg_pool: deadpool_postgres::Pool, expire_interval: Duration) -> Self {
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
        f: impl FnOnce(deadpool_postgres::Client, K) -> FutOne,
    ) -> Result<Option<V>, Error>
    where
        FutOne: Future<Output = Result<Option<V>, Error>>,
    {
        let now = now.unwrap_or(Utc::now());
        match self.map.get(key) {
            Some((value, expire_at)) if now < *expire_at => {
                return Ok(Some(value.clone()));
            }
            _ => {}
        }
        let pg_client = self.pg_pool.get().await?;
        let Some(value) = f(pg_client, key.clone()).await? else {
            return Ok(None);
        };
        self.map
            .insert(key.clone(), (value.clone(), now + self.expire_interval));
        Ok(Some(value))
    }
}
