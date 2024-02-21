use resident_utils::{
    ctrl_c_handler,
    postgres::{deadpool_postgres, make_looper, make_worker},
};
use std::time::Duration;
use tracing::{info, warn, Level};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, Registry};

pub fn get_postgres_pool(url: &str) -> anyhow::Result<deadpool_postgres::Pool> {
    let pg_url = url::Url::parse(url)?;
    let dbname = match pg_url.path_segments() {
        Some(mut res) => res.next(),
        None => Some("web"),
    };
    let pool_config = deadpool_postgres::PoolConfig {
        max_size: 2,
        timeouts: deadpool_postgres::Timeouts {
            wait: Some(Duration::from_secs(2)),
            ..Default::default()
        },
        ..Default::default()
    };
    let cfg = deadpool_postgres::Config {
        user: Some(pg_url.username().to_string()),
        password: pg_url.password().map(|password| password.to_string()),
        dbname: dbname.map(|dbname| dbname.to_string()),
        host: pg_url.host_str().map(|host| host.to_string()),
        pool: Some(pool_config),
        ..Default::default()
    };
    Ok(cfg.create_pool(
        Some(deadpool_postgres::Runtime::Tokio1),
        deadpool_postgres::tokio_postgres::NoTls,
    )?)
}

async fn is_batch(batch_code: &str, pg_conn: &deadpool_postgres::Client) -> anyhow::Result<bool> {
    let stmt = pg_conn
        .prepare("SELECT batch_code FROM resident_set_update_batch(p_batch_code := $1)")
        .await?;
    let res = pg_conn.query(&stmt, &[&batch_code]).await?;
    Ok(!res.is_empty())
}

async fn add_task(
    data_json: &serde_json::Value,
    pg_conn: &deadpool_postgres::Client,
) -> anyhow::Result<()> {
    let stmt = pg_conn
        .prepare("INSERT INTO public.workers (data_json) VALUES ($1)")
        .await?;
    let _ = pg_conn.execute(&stmt, &[&data_json]).await?;
    Ok(())
}

async fn get_task(
    pg_conn: &deadpool_postgres::Client,
) -> anyhow::Result<Option<serde_json::Value>> {
    let stmt = pg_conn
        .prepare("SELECT data_json FROM resident_set_delete_worker()")
        .await?;
    let res = pg_conn.query(&stmt, &[]).await?;
    Ok(if res.is_empty() {
        None
    } else {
        let data_json = res[0].get(0);
        Some(data_json)
    })
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
    let pg_pool = get_postgres_pool(&pg_url)?;

    #[allow(clippy::let_underscore_future)]
    let (_, token) = ctrl_c_handler();

    let handles = vec![
        make_looper(
            pg_pool.clone(),
            token.clone(),
            "*/10 * * * * *",
            Duration::from_secs(60),
            |&now: &_, pg_client: _| async move {
                info!("定期的に処理する何か1 {}", now);
                let pg_client = match pg_client {
                    Ok(client) => client,
                    Err(e) => {
                        warn!("pg_client error={}", e);
                        return;
                    }
                };
                match is_batch("minutely_batch", &pg_client).await {
                    Ok(res) => {
                        if res {
                            // 本当にやりたいバッチ処理
                            add_task(&serde_json::json!({"now": now}), &pg_client)
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
            },
            || async move {
                info!("graceful stop looper 1");
            },
        ),
        make_worker(
            pg_pool.clone(),
            token.clone(),
            Duration::from_secs(60),
            |&now: &_, pg_client: _| async move {
                info!("データがあれば処理する何か1 {}", now);
                let pg_client = match pg_client {
                    Ok(client) => client,
                    Err(e) => {
                        warn!("pg_client error={}", e);
                        return Duration::from_secs(60);
                    }
                };
                match get_task(&pg_client).await {
                    Ok(Some(data_json)) => {
                        info!("data_json={}", data_json);
                        Duration::ZERO
                    }
                    Ok(None) => {
                        info!("no data");
                        Duration::from_secs(60)
                    }
                    Err(e) => {
                        warn!("get_task error={}", e);
                        Duration::from_secs(60)
                    }
                }
            },
            || async move {
                info!("graceful stop worker 1");
            },
        ),
    ];

    for handle in handles {
        handle.await.unwrap();
    }
    Ok(())
}
