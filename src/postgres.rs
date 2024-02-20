#[cfg(feature = "postgres")]
pub use deadpool_postgres;

use std::{future::Future, str::FromStr, time::Duration};

use chrono::prelude::*;
use cron::Schedule;
use tokio::{spawn, task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::info;

pub fn make_looper<Fut1, Fut2>(
    pg_pool: deadpool_postgres::Pool,
    token: CancellationToken,
    expression: &str,
    stop_check_second_count: u64,
    f: impl Fn(&DateTime<Utc>, deadpool_postgres::Client) -> Fut1 + Send + Sync + 'static,
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
                match pg_pool.get().await {
                    Ok(pg_client) => {
                        f(&now, pg_client).await;
                    }
                    Err(e) => {
                        // エラーが出たので、ここでは何もしないで次に期待する
                        info!("get_postgres_client error={}", e);
                    }
                }

                // 次の時間取得
                next_tick = schedule.upcoming(Utc).next().unwrap();
            }

            // 次の時間計算
            sleep(Duration::from_secs(std::cmp::min(
                (next_tick - now).num_seconds() as u64,
                stop_check_second_count,
            )))
            .await;
        }
    })
}

pub fn make_worker<Fut1, Fut2>(
    pg_pool: deadpool_postgres::Pool,
    token: CancellationToken,
    stop_check_second_count: u64,
    error_sleep_count: u64,
    f: impl Fn(&DateTime<Utc>, deadpool_postgres::Client) -> Fut1 + Send + Sync + 'static,
    g: impl Fn() -> Fut2 + Send + Sync + 'static,
) -> JoinHandle<()>
where
    Fut1: Future<Output = u64> + Send,
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
                let next_tick_count = match pg_pool.get().await {
                    Ok(pg_client) => f(&now, pg_client).await,
                    Err(e) => {
                        // エラーが出たので、ここでは何もしないで次に期待する
                        info!(error = ?e, "get postgres client");
                        error_sleep_count
                    }
                };

                // 待つ必要が無いなら次のループに入る
                if next_tick_count == 0 {
                    continue;
                }

                // 次の時間取得
                next_tick = now + Duration::from_secs(next_tick_count);
            }

            // 次の時間計算
            sleep(Duration::from_secs(std::cmp::min(
                (next_tick - now).num_seconds() as u64,
                stop_check_second_count,
            )))
            .await;
        }
    })
}
