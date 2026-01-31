# tokio-muxt

A tiny timer that multiplexes a small set of event deadlines over a single `tokio::time::Sleep`.

`MuxTimer` is useful when you have a fixed, small set of event types and want one async timer that wakes for the next event, returning the event's ordinal and its deadline.

## Features

- One `tokio::time::Sleep` instance for many events.
- Coalesce repeated deadlines for the same event to the earliest or latest one.
- Stack-allocated storage with a const-generic capacity.
- Simple `Future` interface that yields `(ordinal, deadline)`.

## Example

```rust
use std::time::Duration;

use tokio::pin;
use tokio::time::Instant;

use tokio_muxt::{CoalesceMode, MuxTimer};

const EVENT_A: usize = 0;
const EVENT_B: usize = 1;

#[tokio::main]
async fn main() {
    let timer: MuxTimer<2> = MuxTimer::default();
    pin!(timer);

    let start = Instant::now();

    timer
        .as_mut()
        .fire_after(EVENT_A, Duration::from_millis(50), CoalesceMode::Earliest);
    timer
        .as_mut()
        .fire_after(EVENT_B, Duration::from_millis(20), CoalesceMode::Earliest);

    let (event, deadline) = timer.as_mut().await;
    println!("first event = {event}, at +{:?}", deadline.duration_since(start));
}
```

## License

[MIT](./LICENSE)
