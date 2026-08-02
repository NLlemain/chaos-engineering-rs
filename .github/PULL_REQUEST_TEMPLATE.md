## What changed

Describe the behavior and affected platforms.

## Real-world effect

Explain how the target is measurably disrupted. Planned injectors must fail closed.

## Recovery

Explain what is journaled and how interrupted execution is restored.

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features --locked`
- [ ] Disruption and restoration are covered by an integration test.
- [ ] Documentation and capability status are accurate.
