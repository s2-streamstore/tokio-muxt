use std::time::Duration;

use proptest::prelude::*;
use tokio::{
    pin,
    time::{Instant, timeout},
};
use tokio_muxt::{CoalesceMode, MuxTimer};

const N: usize = 4;

#[derive(Debug, Clone)]
enum Op {
    Fire {
        ordinal: usize,
        offset_ms: i64,
        mode: CoalesceMode,
    },
    Cancel {
        ordinal: usize,
    },
    Next,
}

fn op() -> impl Strategy<Value = Op> {
    let mode = prop_oneof![Just(CoalesceMode::Earliest), Just(CoalesceMode::Latest)];
    // Coarse offsets make equal deadlines likely; negative ones are already due.
    let offset_ms = (-2i64..=8).prop_map(|step| step * 25);
    prop_oneof![
        3 => (0..N, offset_ms, mode).prop_map(|(ordinal, offset_ms, mode)| Op::Fire {
            ordinal,
            offset_ms,
            mode
        }),
        1 => (0..N).prop_map(|ordinal| Op::Cancel { ordinal }),
        2 => Just(Op::Next),
    ]
}

fn soonest(model: &[Option<Instant>; N]) -> Option<Instant> {
    model.iter().flatten().min().copied()
}

async fn check(ops: Vec<Op>) {
    let timer: MuxTimer<N> = MuxTimer::new();
    pin!(timer);
    let mut model: [Option<Instant>; N] = [None; N];

    for op in ops {
        match op {
            Op::Fire {
                ordinal,
                offset_ms,
                mode,
            } => {
                let now = Instant::now();
                let offset = Duration::from_millis(offset_ms.unsigned_abs());
                let deadline = if offset_ms < 0 {
                    now - offset
                } else {
                    now + offset
                };
                let applies = model[ordinal].is_none_or(|existing| match mode {
                    CoalesceMode::Earliest => deadline <= existing,
                    CoalesceMode::Latest => deadline >= existing,
                });
                assert_eq!(timer.as_mut().fire_at(ordinal, deadline, mode), applies);
                if applies {
                    model[ordinal] = Some(deadline);
                }
            }
            Op::Cancel { ordinal } => {
                let had_deadline = model[ordinal].take().is_some();
                assert_eq!(timer.as_mut().cancel(ordinal), had_deadline);
            }
            Op::Next => match timeout(Duration::from_secs(60), timer.as_mut()).await {
                Ok((ordinal, deadline)) => {
                    assert_eq!(Some(deadline), soonest(&model));
                    assert_eq!(model[ordinal].take(), Some(deadline));
                    assert!(Instant::now() >= deadline);
                }
                Err(_) => assert_eq!(soonest(&model), None),
            },
        }
        assert_eq!(timer.deadlines(), &model);
        assert_eq!(timer.deadline(), soonest(&model));
        assert_eq!(timer.is_armed(), soonest(&model).is_some());
    }
}

proptest! {
    #[test]
    fn matches_reference_model(ops in prop::collection::vec(op(), 0..64)) {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .expect("runtime")
            .block_on(check(ops));
    }
}
