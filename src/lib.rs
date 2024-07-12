use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use tokio::time::{Instant, Sleep};

/// Timer for a limited set of events that are represented by their ordinals.
/// It multiplexes over a single tokio [Sleep] instance.
/// Deadlines for the same event are coalesced to the sooner one if it has not yet fired.
///
/// Deadlines are stored on a stack-allocated array of size `N`, and the ordinals are used to index into it,
/// so the maximum supported ordinal will be `N - 1`. The implementation is designed for small `N` (think single digits).
///
/// Mapping between ordinals and events is up to the user.
#[derive(Debug)]
pub struct MuxTimer<const N: usize> {
    deadlines: [Option<Instant>; N],
    sleep: Pin<Box<Sleep>>,
    armed: bool,
}

impl<const N: usize> Default for MuxTimer<N> {
    fn default() -> Self {
        Self {
            deadlines: [None; N],
            sleep: Box::pin(tokio::time::sleep(Duration::ZERO)),
            armed: false,
        }
    }
}

impl<const N: usize> MuxTimer<N> {
    /// Fire timer for event with `ordinal` after `timeout` duration.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_after(&mut self, ordinal: impl Into<usize>, timeout: Duration) -> bool {
        self.fire_at(ordinal, Instant::now() + timeout)
    }

    /// Fire timer for event with `ordinal` at `deadline`.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_at(&mut self, ordinal: impl Into<usize>, deadline: Instant) -> bool {
        let ordinal = ordinal.into();
        if let Some(existing_deadline) = &mut self.deadlines[ordinal] {
            if *existing_deadline < deadline {
                return false;
            }
            *existing_deadline = deadline;
        } else {
            self.deadlines[ordinal] = Some(deadline);
        }
        if is_sooner(deadline, self.deadline()) {
            self.arm(deadline);
        }
        true
    }

    fn arm(&mut self, deadline: Instant) {
        self.sleep.as_mut().reset(deadline);
        self.armed = true;
    }

    /// Returns whether the timer is armed.
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Returns the next deadline, if armed.
    pub fn deadline(&self) -> Option<Instant> {
        self.armed.then(|| self.sleep.deadline())
    }

    fn fired(&mut self, at: Instant) -> usize {
        let mut ordinal = None;
        let mut next_deadline = None;
        for i in 0..self.deadlines.len() {
            if let Some(deadline) = self.deadlines[i] {
                if ordinal.is_none() && deadline <= at {
                    self.deadlines[i] = None;
                    ordinal = Some(i);
                } else if is_sooner(deadline, next_deadline) {
                    next_deadline = Some(deadline);
                }
            }
        }
        if let Some(deadline) = next_deadline {
            self.arm(deadline);
        }
        ordinal.expect("cannot be armed without an event deadline")
    }
}

fn is_sooner(candidate: Instant, current: Option<Instant>) -> bool {
    current.map_or(true, |current| candidate < current)
}

/// Wait for the next event and return its ordinal.
/// Panics if the timer is not armed.
impl<const N: usize> Future for MuxTimer<N> {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let deadline = self.deadline().expect("armed");
        match self.sleep.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                self.armed = false;
                Poll::Ready(self.fired(deadline))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::pin;

    use super::MuxTimer;

    const EVENT_A: usize = 0;
    const EVENT_B: usize = 1;
    const EVENT_C: usize = 2;

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn firing_order() {
        let mut timer: MuxTimer<3> = MuxTimer::default();
        assert_eq!(timer.deadline(), None);

        assert!(timer.fire_after(EVENT_C, Duration::from_millis(100)));
        assert!(timer.fire_after(EVENT_B, Duration::from_millis(50)));
        assert!(timer.fire_after(EVENT_A, Duration::from_millis(150)));

        pin!(timer);

        let event = timer.as_mut().await;
        assert_eq!(event, EVENT_B);

        let event = timer.as_mut().await;
        assert_eq!(event, EVENT_C);

        let event = timer.as_mut().await;
        assert_eq!(event, EVENT_A);

        assert_eq!(timer.deadline(), None);
    }

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn rearming() {
        let mut timer: MuxTimer<3> = MuxTimer::default();

        assert!(timer.fire_after(EVENT_A, Duration::from_millis(100)));
        assert!(!timer.fire_after(EVENT_A, Duration::from_millis(200)));
        assert!(timer.fire_after(EVENT_A, Duration::from_millis(50)));

        pin!(timer);

        let event = timer.as_mut().await;
        assert_eq!(event, EVENT_A);
        assert_eq!(timer.deadline(), None);
    }
}
