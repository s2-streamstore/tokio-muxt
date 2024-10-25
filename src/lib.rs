use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Duration,
};

use pin_project::pin_project;

use tokio::time::{Instant, Sleep};

/// Timer for a limited set of events that are represented by their ordinals.
/// It multiplexes over a single tokio [Sleep] instance.
/// Deadlines for the same event are coalesced to the sooner one if it has not yet fired.
///
/// Deadlines are stored on a stack-allocated array of size `N`, and the ordinals are used to index into it,
/// so the maximum supported ordinal will be `N - 1`. The implementation is designed for small `N` (think single digits).
///
/// Mapping between ordinals and events is up to the user.
#[pin_project(project = MuxTimerProj)]
#[derive(Debug)]
pub struct MuxTimer<const N: usize> {
    deadlines: [Option<Instant>; N],
    #[pin]
    sleep: Sleep,
    armed_ordinal: usize,
}

impl<const N: usize> Default for MuxTimer<N> {
    fn default() -> Self {
        Self {
            deadlines: [None; N],
            sleep: tokio::time::sleep(Duration::ZERO),
            armed_ordinal: N,
        }
    }
}

impl<const N: usize> MuxTimer<N> {
    /// Fire timer for event with `ordinal` after `timeout` duration.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_after(self: Pin<&mut Self>, ordinal: impl Into<usize>, timeout: Duration) -> bool {
        self.fire_at(ordinal, Instant::now() + timeout)
    }

    /// Fire timer for event with `ordinal` at `deadline`.
    /// Returns `true` if the timer was armed, `false` if it was already armed for the same event with sooner deadline.
    pub fn fire_at(self: Pin<&mut Self>, ordinal: impl Into<usize>, deadline: Instant) -> bool {
        let ordinal = ordinal.into();
        if self.deadlines[ordinal].is_some_and(|d| d < deadline) {
            return false;
        }
        let current_deadline = self.deadline();
        let mut this = self.project();
        this.deadlines[ordinal] = Some(deadline);
        if current_deadline.map_or(true, |d| deadline < d) {
            this.arm(ordinal, deadline);
        }
        true
    }

    /// Returns whether the timer is armed.
    pub fn is_armed(&self) -> bool {
        self.armed_ordinal < N
    }

    /// Returns the next deadline, if armed.
    pub fn deadline(&self) -> Option<Instant> {
        (self.armed_ordinal < N).then(|| self.sleep.deadline())
    }

    /// Returns all current deadlines, which can be indexed by event ordinals.
    pub fn deadlines(&self) -> &[Option<Instant>; N] {
        &self.deadlines
    }
}

impl<'pin, const N: usize> MuxTimerProj<'pin, N> {
    fn arm(&mut self, ordinal: usize, deadline: Instant) {
        self.sleep.as_mut().reset(deadline);
        *self.armed_ordinal = ordinal;
    }

    fn soonest_event(&self) -> Option<(usize, Instant)> {
        self.deadlines
            .iter()
            .enumerate()
            .filter_map(|(ordinal, slot)| slot.map(|deadline| (ordinal, deadline)))
            .min_by(|(_, x), (_, y)| x.cmp(y))
    }
}

/// Wait for the next event and return its ordinal, along with that event's deadline.
/// Panics if the timer is not armed.
impl<const N: usize> Future for MuxTimer<N> {
    type Output = (usize, Instant);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        assert!(self.armed_ordinal < N);
        let mut this = self.project();
        ready!(this.sleep.as_mut().poll(cx));
        let fired_ordinal = std::mem::replace(this.armed_ordinal, N);
        let fired_deadline = this.deadlines[fired_ordinal].take().expect("armed");
        assert_eq!(fired_deadline, this.sleep.deadline());
        if let Some((ordinal, deadline)) = this.soonest_event() {
            this.arm(ordinal, deadline);
        }
        Poll::Ready((fired_ordinal, fired_deadline))
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
        let timer: MuxTimer<3> = MuxTimer::default();
        pin!(timer);

        assert_eq!(timer.deadline(), None);

        assert!(timer
            .as_mut()
            .fire_after(EVENT_C, Duration::from_millis(100)));
        assert!(timer
            .as_mut()
            .fire_after(EVENT_B, Duration::from_millis(50)));
        assert!(timer
            .as_mut()
            .fire_after(EVENT_A, Duration::from_millis(150)));

        let (event, instant_b) = timer.as_mut().await;
        assert_eq!(event, EVENT_B);

        let (event, instant_c) = timer.as_mut().await;
        assert_eq!(instant_c.duration_since(instant_b).as_millis(), 50);
        assert_eq!(event, EVENT_C);

        let (event, instant_a) = timer.as_mut().await;
        assert_eq!(instant_a.duration_since(instant_c).as_millis(), 50);
        assert_eq!(event, EVENT_A);

        assert_eq!(timer.deadline(), None);
    }

    #[tokio::main(flavor = "current_thread", start_paused = true)]
    #[test]
    async fn rearming() {
        let timer: MuxTimer<3> = MuxTimer::default();
        pin!(timer);

        assert!(timer
            .as_mut()
            .fire_after(EVENT_A, Duration::from_millis(100)));
        assert!(!timer
            .as_mut()
            .fire_after(EVENT_A, Duration::from_millis(200)));
        assert!(timer
            .as_mut()
            .fire_after(EVENT_A, Duration::from_millis(50)));

        let (event, _) = timer.as_mut().await;
        assert_eq!(event, EVENT_A);
        assert_eq!(timer.deadline(), None);
    }
}
