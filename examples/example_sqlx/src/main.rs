use chrono::prelude::*;
use resident_utils::{
    ctrl_c_handler,
    sqlx::{
        make_looper, make_worker,
        sqlx::{postgres::PgPoolOptions, query, query_as},
        SqlxPool,
    },
    LoopState, Schedule,
};
use std::{str::FromStr, time::Duration};
use tracing::{info, warn, Level};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, Registry};
use uuid::Uuid;

pub async fn get_postgres_pool(url: &str) -> anyhow::Result<SqlxPool> {
    let res = PgPoolOptions::new().max_connections(5).connect(url).await?;
    Ok(res)
}

async fn is_batch(batch_code: &str, pg_pool: &SqlxPool) -> anyhow::Result<bool> {
    let res: Option<(String,)> =
        query_as("SELECT batch_code FROM resident_set_update_batch(p_batch_code := $1)")
            .bind(batch_code)
            .fetch_optional(pg_pool)
            .await?;
    Ok(res.is_some())
}

async fn add_task(data_json: &serde_json::Value, pg_pool: &SqlxPool) -> anyhow::Result<()> {
    query("INSERT INTO public.workers (data_json) VALUES ($1)")
        .bind(data_json)
        .execute(pg_pool)
        .await?;

    let now: (DateTime<Utc>, Uuid) = query_as("SELECT now(), gen_random_uuid()")
        .fetch_one(pg_pool)
        .await?;
    info!("add_task now={}, uuid={}", now.0, now.1);
    Ok(())
}

async fn get_task(pg_pool: &SqlxPool) -> anyhow::Result<Option<serde_json::Value>> {
    let res: Option<(serde_json::Value,)> =
        query_as("SELECT data_json FROM resident_set_delete_worker()")
            .fetch_optional(pg_pool)
            .await?;
    Ok(res.map(|(data_json,)| data_json))
}

fn prepare_log(app_name: &str) {
    let formatting_layer = BunyanFormattingLayer::new(app_name.into(), std::io::stdout);
    let filter = Targets::new().with_target(app_name, Level::INFO);
    let subscriber = Registry::default()
        .with(filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    prepare_log("example");
    let pg_url =
        std::env::var("PG_URL").unwrap_or("postgres://user:pass@localhost:5432/web".to_owned());
    let pg_pool = get_postgres_pool(&pg_url).await?;

    #[allow(clippy::let_underscore_future)]
    let (_, token) = ctrl_c_handler();

    let handles = vec![
        make_looper(
            pg_pool.clone(),
            token.clone(),
            Schedule::from_str("*/10 * * * * *").unwrap(),
            Duration::from_secs(10),
            |now, pg_pool| async move {
                info!("定期的に処理する何か1 {}", now);
                match is_batch("minutely_batch", &pg_pool).await {
                    Ok(res) => {
                        if res {
                            // 本当にやりたいバッチ処理
                            add_task(&serde_json::json!({"now": now}), &pg_pool)
                                .await
                                .unwrap();
                        } else {
                            info!("is_batch is false");
                        }
                    }
                    Err(e) => {
                        warn!("is_batch error={}", e);
                    }
                }
                LoopState::Continue
            },
            |_| async move {
                info!("graceful stop looper 1");
            },
        ),
        make_worker(
            pg_pool.clone(),
            token.clone(),
            Duration::from_secs(10),
            |now, pg_pool| async move {
                info!("データがあれば処理する何か1 {}", now);
                match get_task(&pg_pool).await {
                    Ok(Some(data_json)) => {
                        info!("data_json={}", data_json);
                        LoopState::Continue
                    }
                    Ok(None) => {
                        info!("no data");
                        LoopState::Duration(Duration::from_secs(60))
                    }
                    Err(e) => {
                        warn!("get_task error={}", e);
                        LoopState::Duration(Duration::from_secs(60))
                    }
                }
            },
            |_| async move {
                info!("graceful stop worker 1");
            },
        ),
    ];

    for handle in handles {
        handle.await.unwrap();
    }
    Ok(())
}
