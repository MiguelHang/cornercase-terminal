## What

<!-- What does this change do? One or two sentences. -->

## Why

<!-- The problem it solves or the issue it closes (Closes #123). -->

## How

<!-- The approach, and anything a reviewer should look at closely. Mention alternatives you discarded and why. -->

## Testing

<!-- New or updated tests, and what you checked by hand (terminal, size, compact mode…). -->

## Screenshots

<!-- For anything visible: before / after, or a short recording. Delete this section otherwise. -->

## Checklist

- [ ] `cargo fmt`, `cargo clippy --all-targets --all-features --locked -- -D warnings` and `cargo test --locked` pass
- [ ] `cargo-machete` and `npx -y jscpd@4.3.0` pass
- [ ] Changed snapshots were reviewed, not just accepted
- [ ] No new keyboard shortcuts, or they were discussed in an issue first
- [ ] `CLAUDE.md` is updated if this changes a design decision or adds a module
