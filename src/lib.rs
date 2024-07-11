use std::{pin::Pin, time::Duration};

use ordinal_map::{map::OrdinalArrayMap, Ordinal};
use tokio::time::Instant;

/// A multiplexed timer for a limited set of events.
/// Deadlines for the same event are coalesced to the soonest one that has not yet fired.
///
/// Events must implement [ordinal_map::Ordinal].
/// The S type parameter is the number of ordinals and should be specified as { E::ORDINAL_SIZE }.
/// It can be removed after https://github.com/rust-lang/rust/issues/76560.
#[derive(Debug)]
pub struct MuxSleep<E: Ordinal + std::fmt::Debug, const S: usize> {
    deadlines: OrdinalArrayMap<E, Instant, S>,
    sleep: Pin<Box<tokio::time::Sleep>>,
    armed: bool,
}

impl<E: Ordinal + std::fmt::Debug, const S: usize> Default for MuxSleep<E, S> {
    fn default() -> Self {
        Self {
            deadlines: OrdinalArrayMap::new(),
            sleep: Box::pin(tokio::time::sleep(Duration::ZERO)),
            armed: false,
        }
    }
}

impl<E: Ordinal + std::fmt::Debug, const S: usize> MuxSleep<E, S> {
    /// Fire timer for `event` after `timeout` duration.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_after(&mut self, event: E, timeout: Duration) -> bool {
        self.fire_at(event, Instant::now() + timeout)
    }

    /// Fire timer for `event` at `deadline`.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_at(&mut self, event: E, deadline: Instant) -> bool {
        if let Some(existing_deadline) = self.deadlines.get_mut(&event) {
            if *existing_deadline < deadline {
                // already armed with sooner deadline
                return false;
            }
            *existing_deadline = deadline;
        } else {
            self.deadlines.insert(event, deadline);
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

    /// Waits for the next event and returns it.
    /// Panics if the timer is not armed.
    pub async fn next_event(&mut self) -> E {
        let armed_deadline = self.deadline().expect("armed");
        self.sleep.as_mut().await;
        let mut found_event = None;
        let mut next_deadline = None;
        for (event, deadline) in &self.deadlines {
            if found_event.is_none() && *deadline <= armed_deadline {
                found_event = Some(event);
            } else if !next_deadline.is_some_and(|d| d >= *deadline) {
                next_deadline = Some(*deadline);
            }
        }
        let found_event = found_event.expect("cannot be armed without an event deadline");
        self.deadlines.remove(&found_event);
        if let Some(deadline) = next_deadline {
            self.arm(deadline);
        } else {
            self.armed = false;
        }
        found_event
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ordinal_map::Ordinal;

    use super::MuxSleep;

    #[derive(Ordinal, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum Event {
        A,
        B,
    }

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn firing_order() {
        let mut timer: MuxSleep<Event, { Event::ORDINAL_SIZE }> = MuxSleep::default();
        assert_eq!(timer.deadline(), None);

        assert!(timer.fire_after(Event::A, Duration::from_millis(100)));
        assert!(timer.fire_after(Event::B, Duration::from_millis(50)));

        let event = timer.next_event().await;
        assert_eq!(event, Event::B);

        let event = timer.next_event().await;
        assert_eq!(event, Event::A);
        assert_eq!(timer.deadline(), None);
    }

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn rearming() {
        let mut timer: MuxSleep<Event, { Event::ORDINAL_SIZE }> = MuxSleep::default();

        assert!(timer.fire_after(Event::A, Duration::from_millis(100)));
        assert!(!timer.fire_after(Event::A, Duration::from_millis(200)));
        assert!(timer.fire_after(Event::A, Duration::from_millis(50)));

        let event = timer.next_event().await;
        assert_eq!(event, Event::A);
        assert_eq!(timer.deadline(), None);
    }
}
