use std::{pin::Pin, time::Duration};

use tokio::time::Instant;

/// A multiplexed timer for a limited set of events represented by their ordinals.
/// Deadlines for the same event are coalesced to the soonest one that has not yet fired.
///
/// `N` signifies the number of events, and the maximum supported ordinal will be `N - 1`.
///
/// Mapping between ordinals and events is up to the user.
#[derive(Debug)]
pub struct MuxSleep<const N: usize> {
    deadlines: [Option<Instant>; N],
    sleep: Pin<Box<tokio::time::Sleep>>,
    armed: bool,
}

impl<const N: usize> Default for MuxSleep<N> {
    fn default() -> Self {
        assert!(N < 16, "not designed for large N");
        Self {
            deadlines: [None; N],
            sleep: Box::pin(tokio::time::sleep(Duration::ZERO)),
            armed: false,
        }
    }
}

impl<const N: usize> MuxSleep<N> {
    /// Fire timer for event with `ordinal` after `timeout` duration.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_after(&mut self, ordinal: usize, timeout: Duration) -> bool {
        self.fire_at(ordinal, Instant::now() + timeout)
    }

    /// Fire timer for event with `ordinal` at `deadline`.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_at(&mut self, ordinal: usize, deadline: Instant) -> bool {
        if let Some(existing_deadline) = &mut self.deadlines[ordinal] {
            if *existing_deadline < deadline {
                return false;
            }
            *existing_deadline = deadline;
        } else {
            self.deadlines[ordinal] = Some(deadline);
        }
        if !self.deadline().is_some_and(|d| d > self.sleep.deadline()) {
            self.arm(deadline);
        }
        true
    }

    fn arm(&mut self, deadline: Instant) {
        self.sleep.as_mut().reset(deadline);
        self.armed = true;
    }

    /// Returns `true` if the timer is armed.
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Returns the deadline of the next event, if armed.
    pub fn deadline(&self) -> Option<Instant> {
        self.armed.then(|| self.sleep.deadline())
    }

    /// Waits for the next event and returns its ordinal.
    /// Panics if the timer is not armed.
    pub async fn next_event(&mut self) -> usize {
        let armed_deadline = self.deadline().expect("armed");
        self.sleep.as_mut().await;
        let mut ordinal = None;
        let mut next_deadline = None;
        for i in 0..self.deadlines.len() {
            if let Some(deadline) = self.deadlines[i] {
                if ordinal.is_none() && deadline <= armed_deadline {
                    self.deadlines[i] = None;
                    ordinal = Some(i);
                } else if !next_deadline.is_some_and(|d| d >= deadline) {
                    next_deadline = Some(deadline);
                }
            }
        }
        if let Some(deadline) = next_deadline {
            self.arm(deadline);
        } else {
            self.armed = false;
        }
        ordinal.expect("cannot be armed without an event deadline")
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::MuxSleep;

    const EVENT_A: usize = 0;
    const EVENT_B: usize = 1;

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn firing_order() {
        let mut timer: MuxSleep<2> = MuxSleep::default();
        assert_eq!(timer.deadline(), None);

        assert!(timer.fire_after(EVENT_A, Duration::from_millis(100)));
        assert!(timer.fire_after(EVENT_B, Duration::from_millis(50)));

        let event = timer.next_event().await;
        assert_eq!(event, EVENT_B);

        let event = timer.next_event().await;
        assert_eq!(event, EVENT_A);
        assert_eq!(timer.deadline(), None);
    }

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn rearming() {
        let mut timer: MuxSleep<2> = MuxSleep::default();

        assert!(timer.fire_after(EVENT_A, Duration::from_millis(100)));
        assert!(!timer.fire_after(EVENT_A, Duration::from_millis(200)));
        assert!(timer.fire_after(EVENT_A, Duration::from_millis(50)));

        let event = timer.next_event().await;
        assert_eq!(event, EVENT_A);
        assert_eq!(timer.deadline(), None);
    }
}
