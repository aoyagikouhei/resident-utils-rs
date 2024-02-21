#[cfg(feature = "postgres")]
pub use deadpool_postgres;

use std::{future::Future, str::FromStr, time::Duration};

use chrono::prelude::*;
use cron::Schedule;
use tokio::{spawn, task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;

// 次の処理までスリープする
async fn execute_sleep(
    stop_check_duration: &Duration,
    next_tick: &DateTime<Utc>,
    now: &DateTime<Utc>,
) {
    let tick_duration = Duration::from_secs((*next_tick - *now).num_seconds() as u64);
    let duration = if stop_check_duration < &tick_duration {
        stop_check_duration
    } else {
        &tick_duration
    };
    sleep(*duration).await;
}

pub fn make_looper<Fut1, Fut2>(
    pg_pool: deadpool_postgres::Pool,
    token: CancellationToken,
    expression: &str,
    stop_check_duration: Duration,
    f: impl Fn(&DateTime<Utc>, Result<deadpool_postgres::Client, deadpool_postgres::PoolError>) -> Fut1
        + Send
        + Sync
        + 'static,
    g: impl Fn() -> Fut2 + Send + Sync + 'static,
) -> JoinHandle<()>
where
    Fut1: Future<Output = ()> + Send,
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
                f(&now, pg_pool.get().await).await;

                // 次の時間取得
                next_tick = schedule.upcoming(Utc).next().unwrap();
            }

            execute_sleep(&stop_check_duration, &next_tick, &now).await;
        }
    })
}

pub fn make_worker<Fut1, Fut2>(
    pg_pool: deadpool_postgres::Pool,
    token: CancellationToken,
    stop_check_duration: Duration,
    f: impl Fn(&DateTime<Utc>, Result<deadpool_postgres::Client, deadpool_postgres::PoolError>) -> Fut1 + Send + Sync + 'static,
    g: impl Fn() -> Fut2 + Send + Sync + 'static,
) -> JoinHandle<()>
where
    Fut1: Future<Output = Duration> + Send,
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
                let duration = f(&now, pg_pool.get().await).await;

                // 待つ必要が無いなら次のループに入る
                if duration.is_zero() {
                    continue;
                }

                // 次の時間取得
                next_tick = now + duration;
            }

            execute_sleep(&stop_check_duration, &next_tick, &now).await;
        }
    })
}
