pub use deadpool_redis;

use std::{future::Future, str::FromStr, time::Duration};

use chrono::prelude::*;
use cron::Schedule;
use tokio::{spawn, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{execute_sleep, LoopState};

pub fn make_looper<Fut1, Fut2>(
    redis_pool: deadpool_redis::Pool,
    token: CancellationToken,
    expression: &str,
    stop_check_duration: Duration,
    f: impl Fn(DateTime<Utc>, Result<deadpool_redis::Connection, deadpool_redis::PoolError>) -> Fut1
        + Send
        + Sync
        + 'static,
    g: impl Fn() -> Fut2 + Send + Sync + 'static,
) -> JoinHandle<()>
where
    Fut1: Future<Output = LoopState> + Send,
    Fut2: Future<Output = ()> + Send,
{
    let expression = expression.to_owned();
    spawn(async move {
        let schedule = Schedule::from_str(&expression).unwrap();
        let mut next_tick: DateTime<Utc> = schedule.upcoming(Utc).next().unwrap();
        loop {
            // グレースフルストップのチェック
            if token.is_cancelled() {
                g().await;
                break;
            }

            let now = Utc::now();
            if now >= next_tick {
                // 定期的に行う処理実行
                if let Some(res) = f(now, redis_pool.get().await).await.looper(&token, &now, &schedule) {
                    next_tick = res;
                } else {
                    break;
                }
            }

            execute_sleep(&stop_check_duration, &next_tick, &now).await;
        }
    })
}

pub fn make_worker<Fut1, Fut2>(
    redis_pool: deadpool_redis::Pool,
    token: CancellationToken,
    stop_check_duration: Duration,
    f: impl Fn(DateTime<Utc>, Result<deadpool_redis::Connection, deadpool_redis::PoolError>) -> Fut1
        + Send
        + Sync
        + 'static,
    g: impl Fn() -> Fut2 + Send + Sync + 'static,
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
                g().await;
                break;
            }

            // 現在時間と次実行する処理の時間をチェックする
            let now = Utc::now();
            if now >= next_tick {
                // 定期的に行う処理実行
                if let Some(res) = f(now, redis_pool.get().await).await.worker(&token, &now) {
                    next_tick = res;
                } else {
                    break;
                }  
            }

            execute_sleep(&stop_check_duration, &next_tick, &now).await;
        }
    })
}
