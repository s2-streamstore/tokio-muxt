use std::{pin::Pin, time::Duration};

use tokio::time::Instant;

pub trait OrdEvent: Sized {
    const NUM_EVENTS: usize;

    /// Ordinal of the event.
    fn ordinal(&self) -> usize;

    /// Event from ordinal.
    fn from_ordinal(ordinal: usize) -> Option<Self>;
}

/// A multiplexed timer for a limited set of events.
/// Deadlines for the same event are coalesced to the soonest one that has not yet fired.
#[derive(Debug)]
pub struct MuxSleep<E: OrdEvent, const N: usize> {
    deadlines: [Option<Instant>; N],
    sleep: Pin<Box<tokio::time::Sleep>>,
    armed: bool,
    _phantom: std::marker::PhantomData<E>,
}

impl<E: OrdEvent, const N: usize> Default for MuxSleep<E, N> {
    fn default() -> Self {
        assert_eq!(E::NUM_EVENTS, N);
        Self {
            deadlines: [None; N],
            sleep: Box::pin(tokio::time::sleep(Duration::ZERO)),
            armed: false,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<E: OrdEvent, const N: usize> MuxSleep<E, N> {
    /// Fire timer for `event` after `timeout` duration.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_after(&mut self, event: E, timeout: Duration) -> bool {
        self.fire_at(event, Instant::now() + timeout)
    }

    /// Fire timer for `event` at `deadline`.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_at(&mut self, event: E, deadline: Instant) -> bool {
        let ord = event.ordinal();
        if let Some(existing_deadline) = &mut self.deadlines[ord] {
            if *existing_deadline < deadline {
                return false;
            }
            *existing_deadline = deadline;
        } else {
            self.deadlines[ord] = Some(deadline);
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
        let mut event_idx = None;
        let mut next_deadline = None;
        for i in 0..self.deadlines.len() {
            if let Some(deadline) = self.deadlines[i] {
                if event_idx.is_none() && deadline <= armed_deadline {
                    self.deadlines[i] = None;
                    event_idx = Some(i);
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
        E::from_ordinal(event_idx.expect("cannot be armed without an event deadline"))
            .expect("valid ordinal")
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::OrdEvent;

    use super::MuxSleep;

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum SimpleEvent {
        A,
        B,
    }

    impl OrdEvent for SimpleEvent {
        const NUM_EVENTS: usize = 2;

        fn ordinal(&self) -> usize {
            match self {
                SimpleEvent::A => 0,
                SimpleEvent::B => 1,
            }
        }

        fn from_ordinal(ordinal: usize) -> Option<Self> {
            match ordinal {
                0 => Some(SimpleEvent::A),
                1 => Some(SimpleEvent::B),
                _ => None,
            }
        }
    }

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn firing_order() {
        let mut timer: MuxSleep<SimpleEvent, { SimpleEvent::NUM_EVENTS }> = MuxSleep::default();
        assert_eq!(timer.deadline(), None);

        assert!(timer.fire_after(SimpleEvent::A, Duration::from_millis(100)));
        assert!(timer.fire_after(SimpleEvent::B, Duration::from_millis(50)));

        let event = timer.next_event().await;
        assert_eq!(event, SimpleEvent::B);

        let event = timer.next_event().await;
        assert_eq!(event, SimpleEvent::A);
        assert_eq!(timer.deadline(), None);
    }

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn rearming() {
        let mut timer: MuxSleep<SimpleEvent, { SimpleEvent::NUM_EVENTS }> = MuxSleep::default();

        assert!(timer.fire_after(SimpleEvent::A, Duration::from_millis(100)));
        assert!(!timer.fire_after(SimpleEvent::A, Duration::from_millis(200)));
        assert!(timer.fire_after(SimpleEvent::A, Duration::from_millis(50)));

        let event = timer.next_event().await;
        assert_eq!(event, SimpleEvent::A);
        assert_eq!(timer.deadline(), None);
    }
}
