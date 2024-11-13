use std::{future::Future, time::Duration};

use chrono::prelude::*;
use cron::Schedule;
use tokio::{spawn, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{execute_sleep, LoopState};

pub use sqlx;
pub mod holder;
pub type SqlxPool = sqlx::Pool<sqlx::Postgres>;

pub fn make_looper<Fut1, Fut2>(
    pg_pool: SqlxPool,
    token: CancellationToken,
    schedule: Schedule,
    stop_check_duration: Duration,
    task_function: impl Fn(DateTime<Utc>, SqlxPool, CancellationToken) -> Fut1 + Send + Sync + 'static,
    stop_function: impl Fn(SqlxPool) -> Fut2 + Send + Sync + 'static,
) -> JoinHandle<()>
where
    Fut1: Future<Output = LoopState> + Send,
    Fut2: Future<Output = ()> + Send,
{
    spawn(async move {
        let mut next_tick: DateTime<Utc> = match schedule.upcoming(Utc).next() {
            Some(next_tick) => next_tick,
            None => {
                stop_function(pg_pool.clone()).await;
                return;
            }
        };
        loop {
            // グレースフルストップのチェック
            if token.is_cancelled() {
                stop_function(pg_pool.clone()).await;
                break;
            }

            let now = Utc::now();
            if now >= next_tick {
                // 定期的に行う処理実行
                if let Some(res) = task_function(now, pg_pool.clone(), token.clone())
                    .await
                    .looper(&token, &now, &schedule)
                {
                    next_tick = res;
                } else {
                    stop_function(pg_pool.clone()).await;
                    break;
                }
            }

            execute_sleep(&stop_check_duration, &next_tick, &now).await;
        }
    })
}

pub fn make_worker<Fut1, Fut2>(
    pg_pool: SqlxPool,
    token: CancellationToken,
    stop_check_duration: Duration,
    task_function: impl Fn(DateTime<Utc>, SqlxPool, CancellationToken) -> Fut1 + Send + Sync + 'static,
    stop_function: impl Fn(SqlxPool) -> Fut2 + Send + Sync + 'static,
) -> JoinHandle<()>
where
    Fut1: Future<Output = LoopState> + Send,
    Fut2: Future<Output = ()> + Send,
{
    spawn(async move {
        // 動き出した瞬間は実行する
        let mut next_tick: DateTime<Utc> = Utc::now();
        loop {
            // グレースフルストップのチェック
            if token.is_cancelled() {
                stop_function(pg_pool.clone()).await;
                break;
            }

            // 現在時間と次実行する処理の時間をチェックする
            let now = Utc::now();
            if now >= next_tick {
                // 定期的に行う処理実行
                if let Some(res) = task_function(now, pg_pool.clone(), token.clone())
                    .await
                    .worker(&token, &now)
                {
                    next_tick = res;
                } else {
                    stop_function(pg_pool.clone()).await;
                    break;
                }
            }

            execute_sleep(&stop_check_duration, &next_tick, &now).await;
        }
    })
}