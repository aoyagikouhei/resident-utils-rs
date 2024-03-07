use std::time::Duration;

pub type PgPool = resident_utils::postgres::deadpool_postgres::Pool;
pub type PgClient = resident_utils::postgres::deadpool_postgres::Client;

pub fn get_postgres_pool(url: &str) -> anyhow::Result<PgPool> {
    let pg_url = url::Url::parse(url)?;
    let dbname = match pg_url.path_segments() {
        Some(mut res) => res.next(),
        None => Some("web"),
    };
    let pool_config = resident_utils::postgres::deadpool_postgres::PoolConfig {
        max_size: 2,
        timeouts: resident_utils::postgres::deadpool_postgres::Timeouts {
            wait: Some(Duration::from_secs(2)),
            ..Default::default()
        },
        ..Default::default()
    };
    let cfg = resident_utils::postgres::deadpool_postgres::Config {
        user: Some(pg_url.username().to_string()),
        password: pg_url.password().map(|password| password.to_string()),
        dbname: dbname.map(|dbname| dbname.to_string()),
        host: pg_url.host_str().map(|host| host.to_string()),
        pool: Some(pool_config),
        ..Default::default()
    };
    Ok(cfg.create_pool(
        Some(resident_utils::postgres::deadpool_postgres::Runtime::Tokio1),
        resident_utils::postgres::deadpool_postgres::tokio_postgres::NoTls,
    )?)
}
