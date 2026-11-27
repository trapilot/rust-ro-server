#![allow(dead_code)]
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub const LATCH_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct CountDownLatch {
    count: Arc<Mutex<i32>>,
    cvar: Arc<Condvar>,
}

impl CountDownLatch {
    pub fn new(count: usize) -> Self {
        Self {
            count: Arc::new(Mutex::new(count as i32)),
            cvar: Arc::new(Condvar::new()),
        }
    }

    pub fn countdown(&self) {
        let mut count_guard = self.count.lock().unwrap();
        if *count_guard > 0 {
            *count_guard -= 1;
        }
        self.cvar.notify_one();
    }

    pub fn wait(&self) {
        let _unused = self.cvar.wait_while(self.count.lock().unwrap(), |count| *count > 0).unwrap();
    }

    pub fn wait_with_timeout(&self, duration: Duration) {
        let (lock, wait_timeout) = self
            .cvar
            .wait_timeout_while(self.count.lock().unwrap(), duration, |count| *count > 0)
            .unwrap();
        if wait_timeout.timed_out() {
            panic!("countdown latch timed out after {duration:?}, {} countdown(s) remaining", *lock);
        }
    }
}

#[derive(Debug, Clone)]
pub struct IncrementLatch {
    count: Arc<Mutex<usize>>,
    cvar: Arc<Condvar>,
}

impl IncrementLatch {
    pub fn new() -> Self {
        Self {
            count: Arc::new(Mutex::new(0)),
            cvar: Arc::new(Condvar::new()),
        }
    }

    pub fn increment(&self) {
        let mut count_guard = self.count.lock().unwrap();
        *count_guard += 1;
        self.cvar.notify_one();
    }

    pub fn reset(&self) {
        let mut count_guard = self.count.lock().unwrap();
        *count_guard = 0;
    }

    pub fn wait_expected_count(&self, expected_count: usize) {
        let _unused = self
            .cvar
            .wait_while(self.count.lock().unwrap(), |count| *count < expected_count)
            .unwrap();
    }

    pub fn wait_expected_count_with_timeout(&self, expected_count: usize, duration: Duration) {
        let (lock, wait_timeout) = self
            .cvar
            .wait_timeout_while(self.count.lock().unwrap(), duration, |count| *count < expected_count)
            .unwrap();
        if wait_timeout.timed_out() {
            panic!(
                "increment latch timed out after {duration:?}: count is {}, expected at least {expected_count}",
                *lock
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::tests::common::sync_helper::{CountDownLatch, IncrementLatch};

    #[test]
    #[should_panic]
    fn test_countdown_should_wait_until_timeout_if_condition_not_satisfied() {
        // Given
        let countdown_latch = CountDownLatch::new(10);
        let count_down_latch_cloned = countdown_latch.clone();
        let start = Instant::now();
        // When
        thread::spawn(move || count_down_latch_cloned.countdown());
        // Then
        countdown_latch.wait_with_timeout(Duration::from_millis(20));
        assert!(start.elapsed().as_millis() >= 20);
    }

    #[test]
    fn test_countdown_should_wait_when_condition_is_satisfied_before_timeout() {
        // Given
        let countdown_latch = CountDownLatch::new(1);
        let count_down_latch_cloned = countdown_latch.clone();
        let start = Instant::now();
        // When
        thread::spawn(move || count_down_latch_cloned.countdown());
        // Then
        countdown_latch.wait_with_timeout(Duration::from_secs(2));
        assert!(start.elapsed().as_secs() < 1);
    }

    #[test]
    #[should_panic]
    fn test_increment_should_wait_until_timeout_if_condition_not_satisfied() {
        // Given
        let increment_latch = IncrementLatch::new();
        let increment_latch_cloned = increment_latch.clone();
        let start = Instant::now();
        // When
        thread::spawn(move || increment_latch_cloned.increment());
        // Then
        increment_latch.wait_expected_count_with_timeout(2, Duration::from_millis(200));
        assert!(start.elapsed().as_millis() >= 200);
    }

    #[test]
    fn test_increment_should_return_immediately_when_count_exceeds_expected() {
        // Given
        let increment_latch = IncrementLatch::new();
        increment_latch.increment();
        increment_latch.increment();
        increment_latch.increment();
        let start = Instant::now();
        // When
        increment_latch.wait_expected_count_with_timeout(2, Duration::from_secs(2));
        // Then
        assert!(start.elapsed().as_secs() < 1);
    }

    #[test]
    fn test_increment_should_wait_when_condition_is_satisfied_before_timeout() {
        // Given
        let increment_latch = IncrementLatch::new();
        let increment_latch_cloned = increment_latch.clone();
        let start = Instant::now();
        // When
        thread::spawn(move || increment_latch_cloned.increment());
        // Then
        increment_latch.wait_expected_count_with_timeout(1, Duration::from_secs(2));
        assert!(start.elapsed().as_secs() < 1);
    }
}
