use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Test clock with a fixed, manually adjustable time (millisecond precision).
#[derive(Debug)]
pub struct MockClock {
    millis: AtomicI64,
}

impl MockClock {
    pub fn new(now: DateTime<Utc>) -> Arc<Self> {
        Arc::new(Self {
            millis: AtomicI64::new(now.timestamp_millis()),
        })
    }

    pub fn set(&self, now: DateTime<Utc>) {
        self.millis.store(now.timestamp_millis(), Ordering::SeqCst);
    }

    pub fn advance_seconds(&self, secs: i64) {
        self.millis.fetch_add(secs * 1000, Ordering::SeqCst);
    }
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.millis.load(Ordering::SeqCst))
            .expect("mock clock timestamp out of range")
    }
}
