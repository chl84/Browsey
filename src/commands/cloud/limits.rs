use super::error::{CloudCommandErrorCode, CloudCommandResult};
use std::collections::HashMap;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::debug;

const CLOUD_REMOTE_MAX_CONCURRENT_OPS: usize = 2;
const CLOUD_REMOTE_RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
struct CloudRemoteOpLimiter {
    state: Mutex<CloudRemoteOpState>,
    cv: Condvar,
}

#[derive(Debug, Default)]
struct CloudRemoteOpState {
    counts: HashMap<String, usize>,
    cooldown_until: HashMap<String, Instant>,
}

#[derive(Debug)]
pub(crate) struct CloudRemotePermitGuard {
    pub(crate) remotes: Vec<String>,
}

fn cloud_remote_op_limiter() -> &'static CloudRemoteOpLimiter {
    static LIMITER: OnceLock<CloudRemoteOpLimiter> = OnceLock::new();
    LIMITER.get_or_init(CloudRemoteOpLimiter::default)
}

pub(crate) fn with_cloud_remote_permits<T>(
    mut remotes: Vec<String>,
    f: impl FnOnce() -> CloudCommandResult<T>,
) -> CloudCommandResult<T> {
    remotes.sort();
    remotes.dedup();
    let guard = acquire_cloud_remote_permits(remotes);
    let result = f();
    if let Err(error) = &result {
        if error.code() == CloudCommandErrorCode::RateLimited {
            note_remote_rate_limit_cooldown(&guard.remotes);
        }
    }
    result
}

pub(crate) fn acquire_cloud_remote_permits(remotes: Vec<String>) -> CloudRemotePermitGuard {
    if remotes.is_empty() {
        return CloudRemotePermitGuard { remotes };
    }
    let limiter = cloud_remote_op_limiter();
    let mut state = match limiter.state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    loop {
        let now = Instant::now();
        state.cooldown_until.retain(|_, deadline| *deadline > now);
        let has_capacity = remotes.iter().all(|remote| {
            state.counts.get(remote).copied().unwrap_or(0) < CLOUD_REMOTE_MAX_CONCURRENT_OPS
        });
        let next_cooldown_deadline = remotes
            .iter()
            .filter_map(|remote| state.cooldown_until.get(remote).copied())
            .min();
        if has_capacity && next_cooldown_deadline.is_none() {
            for remote in &remotes {
                *state.counts.entry(remote.clone()).or_insert(0) += 1;
            }
            return CloudRemotePermitGuard { remotes };
        }
        if let Some(deadline) = next_cooldown_deadline {
            let wait = deadline.saturating_duration_since(now);
            state = match limiter.cv.wait_timeout(state, wait) {
                Ok((guard, _)) => guard,
                Err(poisoned) => poisoned.into_inner().0,
            };
        } else {
            state = match limiter.cv.wait(state) {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }
}

pub(crate) fn note_remote_rate_limit_cooldown(remotes: &[String]) {
    if remotes.is_empty() {
        return;
    }
    let limiter = cloud_remote_op_limiter();
    let now = Instant::now();
    let cooldown_deadline = now + CLOUD_REMOTE_RATE_LIMIT_COOLDOWN;
    let mut state = match limiter.state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    for remote in remotes {
        let entry = state
            .cooldown_until
            .entry(remote.clone())
            .or_insert(cooldown_deadline);
        if *entry < cooldown_deadline {
            *entry = cooldown_deadline;
        }
    }
    drop(state);
    limiter.cv.notify_all();
    debug!(
        remotes = ?remotes,
        cooldown_ms = CLOUD_REMOTE_RATE_LIMIT_COOLDOWN.as_millis() as u64,
        "applied cloud remote rate-limit cooldown"
    );
}

impl Drop for CloudRemotePermitGuard {
    fn drop(&mut self) {
        if self.remotes.is_empty() {
            return;
        }
        let limiter = cloud_remote_op_limiter();
        let mut state = match limiter.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        for remote in &self.remotes {
            match state.counts.get_mut(remote) {
                Some(count) if *count > 1 => *count -= 1,
                Some(_) => {
                    state.counts.remove(remote);
                }
                None => {}
            }
        }
        limiter.cv.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::{acquire_cloud_remote_permits, cloud_remote_op_limiter, with_cloud_remote_permits};
    use crate::commands::cloud::error::{CloudCommandError, CloudCommandErrorCode};
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Barrier,
        },
        thread,
        time::{Duration, Instant},
    };

    fn unique_test_remote(prefix: &str) -> String {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        format!("{prefix}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn remote_permit_limiter_bounds_same_remote_concurrency() {
        let barrier = Arc::new(Barrier::new(4));
        let active = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..3 {
            let barrier = Arc::clone(&barrier);
            let active = Arc::clone(&active);
            let max_seen = Arc::clone(&max_seen);
            handles.push(thread::spawn(move || {
                barrier.wait();
                let _permit = acquire_cloud_remote_permits(vec!["test-remote".to_string()]);
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                loop {
                    let prev = max_seen.load(Ordering::SeqCst);
                    if current <= prev {
                        break;
                    }
                    if max_seen
                        .compare_exchange(prev, current, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(40));
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        barrier.wait();
        for handle in handles {
            handle.join().expect("thread should finish");
        }

        assert!(max_seen.load(Ordering::SeqCst) <= super::CLOUD_REMOTE_MAX_CONCURRENT_OPS);
    }

    #[test]
    fn rate_limited_result_applies_remote_cooldown() {
        let remote = unique_test_remote("cooldown-remote");

        let result: Result<(), CloudCommandError> =
            with_cloud_remote_permits(vec![remote.clone()], || {
                Err(CloudCommandError::new(
                    CloudCommandErrorCode::RateLimited,
                    "provider rate limit",
                ))
            });
        let err = result.expect_err("expected rate-limited error");
        assert_eq!(err.code(), CloudCommandErrorCode::RateLimited);

        let limiter = cloud_remote_op_limiter();
        let state = limiter.state.lock().expect("limiter lock");
        let deadline = state
            .cooldown_until
            .get(&remote)
            .copied()
            .expect("cooldown deadline should be set");
        assert!(deadline > Instant::now());
    }

    #[test]
    fn acquire_remote_permit_waits_for_active_cooldown_window() {
        let remote = unique_test_remote("cooldown-wait-remote");
        let limiter = cloud_remote_op_limiter();
        {
            let mut state = limiter.state.lock().expect("limiter lock");
            state
                .cooldown_until
                .insert(remote.clone(), Instant::now() + Duration::from_millis(120));
        }
        let started = Instant::now();
        let _permit = acquire_cloud_remote_permits(vec![remote]);
        let waited = started.elapsed();
        assert!(
            waited >= Duration::from_millis(100),
            "expected to wait for cooldown, waited only {:?}",
            waited
        );
    }
}
