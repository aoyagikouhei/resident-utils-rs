#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(all(feature = "postgres", feature = "redis"))]
pub mod postgres_redis;

use chrono::prelude::*;
use cron::Schedule;
use std::{future::Future, str::FromStr, time::Duration};
use tokio::{signal::ctrl_c, spawn, task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::debug;

///
/// LoopState
///   AllTerminate: stop all threads
///   Continue: continue loop
///   Terminate: terminate this loop
///   Duration(duration): sleep duration
///
pub enum LoopState {
    AllTerminate,
    Continue,
    Terminate,
    Duration(Duration),
}

impl LoopState {
    pub(crate) fn looper(&self, token: &CancellationToken, now: &DateTime<Utc>, schedule: &Schedule) -> Option<DateTime<Utc>> {
        match self {
            LoopState::AllTerminate => {
                token.cancel();
                None
            },
            LoopState::Terminate => {
                None
            },
            LoopState::Duration(duration) => {
                // 指定時間待つ
                Some(*now + *duration)
            },
            LoopState::Continue => {
                // 次の時間取得
                Some(schedule.upcoming(Utc).next().unwrap())
            }
        }
    }
    pub(crate) fn worker(&self, token: &CancellationToken, now: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            LoopState::AllTerminate => {
                token.cancel();
                None
            },
            LoopState::Terminate => {
                None
            },
            LoopState::Duration(duration) => {
                // 指定時間待つ
                Some(*now + *duration)
            },
            LoopState::Continue => {
                // 次の時間取得
                Some(*now)
            }
        }
    }

}

// 次の処理までスリープする
#[allow(dead_code)]
pub(crate) async fn execute_sleep(
    stop_check_duration: &Duration,
    next_tick: &DateTime<Utc>,
    now: &DateTime<Utc>,
) {
    // next_tickが過去ならsleepせずに終了
    if now >= next_tick {
        return;
    }

    // 上記でチェックしているので、as u64で問題無い。
    let tick_duration = Duration::from_secs((*next_tick - *now).num_seconds() as u64);
    let duration = if stop_check_duration < &tick_duration {
        stop_check_duration
    } else {
        &tick_duration
    };
    sleep(*duration).await;
}

pub fn ctrl_c_handler() -> (JoinHandle<()>, CancellationToken) {
    let token = CancellationToken::new();
    let cloned_token = token.clone();
    (
        spawn(async move {
            ctrl_c().await.unwrap();
            debug!("received ctrl-c");
            cloned_token.cancel();
        }),
        token,
    )
}

pub fn make_looper<Fut1, Fut2>(
    token: CancellationToken,
    expression: &str,
    stop_check_duration: Duration,
    f: impl Fn(DateTime<Utc>) -> Fut1
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
                if let Some(res) = f(now).await.looper(&token, &now, &schedule) {
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
    token: CancellationToken,
    stop_check_duration: Duration,
    f: impl Fn(DateTime<Utc>) -> Fut1
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
                if let Some(res) = f(now).await.worker(&token, &now) {
                    next_tick = res;
                } else {
                    break;
                }                
            }

            execute_sleep(&stop_check_duration, &next_tick, &now).await;
        }
    })
}
