use chrono::prelude::*;
use resident_utils::{ctrl_c_handler, make_looper, LoopState};
use std::time::Duration;
use tracing::{info, warn, Level};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{filter::Targets, prelude::*, Registry};
use twapi_v2::api::post_2_tweets;
use twapi_v2::oauth10a::OAuthAuthentication;

fn prepare_log(app_name: &str) {
    let formatting_layer = BunyanFormattingLayer::new(app_name.into(), std::io::stdout);
    let filter = Targets::new().with_target(app_name, Level::INFO);
    let subscriber = Registry::default()
        .with(filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

async fn execute_tweet(now: DateTime<Utc>) -> anyhow::Result<()> {
    let auth = OAuthAuthentication::new(
        std::env::var("CONSUMER_KEY").unwrap_or_default(),
        std::env::var("CONSUMER_SECRET").unwrap_or_default(),
        std::env::var("ACCESS_KEY").unwrap_or_default(),
        std::env::var("ACCESS_SECRET").unwrap_or_default(),
    );
    let body = post_2_tweets::Body {
        text: Some(format!("botです。{}", now)),
        ..Default::default()
    };
    match post_2_tweets::Api::new(body).execute(&auth).await {
        Ok((response, rate_limit)) => {
            info!(response = ?response, rate_limit = ?rate_limit, "post tweet success");
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    prepare_log("bot");

    let (_, token) = ctrl_c_handler();
    let handles = vec![make_looper(
        token.clone(),
        "0 15,45 * * * *",
        Duration::from_secs(10),
        |now: _| async move {
            if let Err(err) = execute_tweet(now).await {
                warn!(error = ?err, "execute tweet failed");
            }
            LoopState::Continue
        },
        || async move {
            info!("graceful stop looper 1");
        },
    )];
    for handle in handles {
        handle.await.unwrap();
    }
    Ok(())
}
