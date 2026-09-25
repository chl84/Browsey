use super::{ThumbnailError, ThumbnailErrorCode, ThumbnailResult};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(super) struct Control {
    pub(super) cancelled: Arc<AtomicBool>,
    deadline: Instant,
}

impl Control {
    pub(super) fn new(cancelled: Arc<AtomicBool>, budget: Duration) -> Self {
        Self {
            cancelled,
            deadline: Instant::now() + budget,
        }
    }

    pub(super) fn check(&self) -> ThumbnailResult<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(ThumbnailError::new(
                ThumbnailErrorCode::Cancelled,
                "Thumbnail cancelled",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(ThumbnailError::new(
                ThumbnailErrorCode::TimedOut,
                "Thumbnail timed out",
            ));
        }
        Ok(())
    }

    pub(super) fn remaining(&self, limit: Duration) -> ThumbnailResult<Duration> {
        self.check()?;
        Ok(limit.min(self.deadline.saturating_duration_since(Instant::now())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadline_and_cancellation_are_distinct() {
        let flag = Arc::new(AtomicBool::new(false));
        let control = Control::new(flag.clone(), Duration::ZERO);
        assert_eq!(
            control.check().unwrap_err().code(),
            ThumbnailErrorCode::TimedOut
        );
        flag.store(true, Ordering::Relaxed);
        assert_eq!(
            control.check().unwrap_err().code(),
            ThumbnailErrorCode::Cancelled
        );
    }

    #[test]
    fn fallback_cannot_restart_the_budget() {
        let control = Control::new(Arc::new(AtomicBool::new(false)), Duration::from_millis(50));
        assert!(control.remaining(Duration::from_secs(8)).unwrap() <= Duration::from_millis(50));
    }
}
