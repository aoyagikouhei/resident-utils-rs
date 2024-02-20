#[cfg(feature = "postgres")]
pub mod postgres;

use tokio::{signal::ctrl_c, spawn, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::debug;

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
