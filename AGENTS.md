# AGENTS.md

## Scope
- The core logic lives in `src/lib.rs`.
- Prefer small, focused changes.

## Testing
- Use `cargo nextest run` for full test runs.
- If nextest is unavailable, use `cargo test`.

## Test style
- Timer tests should use `#[tokio::main(flavor = "current_thread", start_paused = true)]`.
- Use `tokio::time::Instant` and fixed `Duration` values for determinism.

## Commits
- Use conventional commits (e.g. `test: add cancel edge cases`).
