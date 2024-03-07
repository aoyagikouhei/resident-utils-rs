use std::{str::FromStr, sync::OnceLock, time::Duration};

use account::Account;
use pg::get_postgres_pool;
use resident_utils::postgres::holder::{HolderMap, HolderMapEachExpire};
use tokio::{sync::Mutex, time::sleep};
use uuid::Uuid;

pub mod account;
pub mod pg;

static HOLDER: OnceLock<Mutex<HolderMap<Uuid, Account>>> = OnceLock::new();
static HOLDER_EACHEXPIRE: OnceLock<Mutex<HolderMapEachExpire<Uuid, Account>>> = OnceLock::new();

async fn get_account(uuid: &Uuid) -> anyhow::Result<Option<Account>> {
    let mut holder = HOLDER.get().unwrap().lock().await;
    let account = holder
        .get(
            uuid,
            None,
            |pg_client, uuid| async move {
                println!("one");
                Account::get_one(&pg_client, &uuid).await
            },
            |pg_client| async move {
                println!("all");
                Account::get_all(&pg_client).await
            },
        )
        .await
        .unwrap();
    Ok(account)
}

async fn get_account_each_expire(uuid: &Uuid) -> anyhow::Result<Option<Account>> {
    let mut holder = HOLDER_EACHEXPIRE.get().unwrap().lock().await;
    let account = holder
        .get(uuid, None, |pg_client, uuid| async move {
            println!("each expire one");
            Account::get_one(&pg_client, &uuid).await
        })
        .await
        .unwrap();
    Ok(account)
}

async fn get_accounts() {
    let account = get_account(&Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap())
        .await
        .unwrap();
    println!("{:?}", account);
    let account = get_account(&Uuid::from_str("00000000-0000-0000-0000-000000000002").unwrap())
        .await
        .unwrap();
    println!("{:?}", account);
    let account = get_account(&Uuid::from_str("00000000-0000-0000-0000-000000000003").unwrap())
        .await
        .unwrap();
    println!("{:?}", account);
    let account = get_account(&Uuid::from_str("00000000-0000-0000-0000-000000000004").unwrap())
        .await
        .unwrap();
    println!("{:?}", account);
    let account = get_account(&Uuid::from_str("00000000-0000-0000-0000-000000000005").unwrap())
        .await
        .unwrap();
    println!("{:?}", account);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pg_url =
        std::env::var("PG_URL").unwrap_or("postgres://user:pass@localhost:5432/web".to_owned());
    let pg_pool = get_postgres_pool(&pg_url)?;

    // スレッドが始まる前に初期化する
    let _ = HOLDER.get_or_init(|| {
        Mutex::new(HolderMap::new(
            pg_pool.clone(),
            Duration::from_secs(3),
            None,
        ))
    });
    let _ = HOLDER_EACHEXPIRE.get_or_init(|| {
        Mutex::new(HolderMapEachExpire::new(
            pg_pool.clone(),
            Duration::from_secs(3),
        ))
    });

    let thread1 = tokio::spawn(async move {
        get_accounts().await;
        sleep(Duration::from_secs(4)).await;
        get_accounts().await;
    });

    let thread2 = tokio::spawn(async move {
        get_accounts().await;
    });

    thread1.await?;
    thread2.await?;

    let thread3 = tokio::spawn(async move {
        let account = get_account_each_expire(
            &Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap(),
        )
        .await
        .unwrap();
        println!("{:?}", account);
        let account = get_account_each_expire(
            &Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap(),
        )
        .await
        .unwrap();
        println!("{:?}", account);
        sleep(Duration::from_secs(4)).await;
        let account = get_account_each_expire(
            &Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap(),
        )
        .await
        .unwrap();
        println!("{:?}", account);
    });

    thread3.await?;

    Ok(())
}
