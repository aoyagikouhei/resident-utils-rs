use std::time::Duration;
use tokio::time::timeout;

pub struct RetryResult<T, E> {
    pub success: Option<T>,
    pub errors: Vec<E>,
    pub timeout_count: u64,
}

pub async fn execute_retry<T, E, Fut>(
    max_try_count: u64,
    retry_duration: Duration,
    timeout_duration: Duration,
    inner: impl Fn(u64) -> Fut,
) -> RetryResult<T, E>
where
    Fut: std::future::Future<Output = Result<T, E>>,
{
    execute_retry_with_exponential_backoff(max_try_count, retry_duration, timeout_duration, inner, false).await
}

pub async fn execute_retry_with_exponential_backoff<T, E, Fut>(
    max_try_count: u64,
    retry_duration: Duration,
    timeout_duration: Duration,
    inner: impl Fn(u64) -> Fut,
    exponential_backoff: bool,
) -> RetryResult<T, E>
where
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut try_count = 0;
    let mut timeout_count = 0;
    let mut errors = vec![];
    loop {
        try_count += 1;
        if timeout_duration.is_zero() {
            match inner(try_count).await {
                Ok(res) => {
                    return RetryResult {
                        success: Some(res),
                        errors,
                        timeout_count,
                    }
                }
                Err(err) => {
                    errors.push(err);
                    
                }
            }
        } else {
            match timeout(timeout_duration, inner(try_count)).await {
                Ok(res) => match res {
                    Ok(res) => {
                        return RetryResult {
                            success: Some(res),
                            errors,
                            timeout_count,
                        }
                    }
                    Err(err) => {
                        errors.push(err);
                    }
                },
                Err(_) => {
                    timeout_count += 1;
                }
            }
        }
        if try_count >= max_try_count {
            return RetryResult {
                success: None,
                errors,
                timeout_count,
            };
        }
        if !retry_duration.is_zero() {
            let duration = if exponential_backoff {
                retry_duration.mul_f64(2_i32.pow(try_count as u32) as f64)
            } else {
                retry_duration
            };
            tokio::time::sleep(duration).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use tokio::time::sleep;

    use super::*;
    // REALM_CODE=test cargo test test_retry -- --nocapture --test-threads=1

    async fn inner_success() -> Result<usize, String> {
        Ok(1)
    }

    async fn inner_fail(n: u64) -> Result<usize, String> {
        println!("inner_fail {}", n);
        Err("error".to_string())
    }

    async fn inner_later() -> Result<usize, String> {
        sleep(Duration::from_millis(100)).await;
        Ok(1)
    }

    async fn inner_complex(n: u64) -> Result<usize, String> {
        if n == 3 {
            Ok(1)
        } else {
            Err("error".to_string())
        }
    }

    #[tokio::test]
    async fn test_retry() -> anyhow::Result<()> {
        // Success
        let res = execute_retry(
            3,
            Duration::from_secs(0),
            Duration::from_secs(0),
            |_n| async { inner_success().await },
        )
        .await;
        assert_eq!(res.success, Some(1));
        assert_eq!(res.errors.len(), 0);
        assert_eq!(res.timeout_count, 0);

        // Failure
        let res = execute_retry_with_exponential_backoff(
            3,
            Duration::from_secs(1),
            Duration::from_secs(0),
            |n| async move { inner_fail(n).await },
            true,
        )
        .await;
        assert_eq!(res.success, None);
        assert_eq!(
            res.errors,
            vec!["error".to_owned(), "error".to_owned(), "error".to_owned(),]
        );
        assert_eq!(res.timeout_count, 0);

        // Timeout
        let res = execute_retry(
            3,
            Duration::from_secs(0),
            Duration::from_millis(10),
            |_n| async { inner_later().await },
        )
        .await;
        assert_eq!(res.success, None);
        assert_eq!(res.errors.len(), 0);
        assert_eq!(res.timeout_count, 3);

        // Complex
        let res = execute_retry(
            3,
            Duration::from_secs(0),
            Duration::from_secs(0),
            |n| async move { inner_complex(n).await },
        )
        .await;
        assert_eq!(res.success, Some(1));
        assert_eq!(res.errors, vec!["error".to_owned(), "error".to_owned()]);
        assert_eq!(res.timeout_count, 0);

        Ok(())
    }
}
