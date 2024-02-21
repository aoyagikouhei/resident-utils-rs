#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "redis")]
pub mod redis;

use chrono::prelude::*;
use std::time::Duration;
use tokio::{signal::ctrl_c, spawn, task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::debug;

// 次の処理までスリープする
#[allow(dead_code)]
pub(crate) async fn execute_sleep(
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
