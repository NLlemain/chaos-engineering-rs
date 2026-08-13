# Contributing

Thanks for improving `chaos-engineering-rs`. The project values real, recoverable effects over a large list of names.

## Start Here

1. Check the [roadmap](docs/ROADMAP.md), [issues](https://github.com/Ninian-Lemain/chaos-engineering-rs/issues), and [discussions](https://github.com/Ninian-Lemain/chaos-engineering-rs/discussions).
2. For a new protocol, open a scenario-pack proposal before adding a core injector.
3. Keep changes focused and include the target operating systems in the pull request.

## Development

Rust 1.82 or newer is required.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --locked
```

Docker-gated integration tests run when a daemon is available:

```bash
CHAOS_RUN_DOCKER_TESTS=1 cargo test -p chaos_core --test docker_integration
```

On PowerShell, use `$env:CHAOS_RUN_DOCKER_TESTS='1'`.

## Injector Contract

Every injector must:

- Declare exactly one status: stable, experimental, or planned.
- Validate commands, permissions, target existence, and nonzero effect parameters before injection.
- Return an error when it cannot produce a measurable disruption.
- Journal recoverable state before applying the effect.
- Make `remove` idempotent where practical.
- Include an integration test proving disruption and restoration.

Planned injectors may reserve configuration and documentation, but must not return a successful simulated handle.

## Scenario Packs

Prefer a pack when behavior can be expressed using HTTP, DNS, TLS, TCP, database, container, Windows, or offline pipeline replay primitives. A pack contribution includes:

- One or more validated scenario files.
- A catalog entry with kind, category, status, protocols, requirements, and source path.
- A short category README update.
- A deterministic local test target or a documented opt-in integration test.

Run catalog tests before opening a pull request:

```bash
cargo test -p chaos_packs
chaos pack list --json
```

## Pull Requests

Describe the real-world effect, how recovery works, and the commands used to verify it. CI must be green; platform-specific behavior should be explicit rather than silently skipped.

Security issues should follow [SECURITY.md](SECURITY.md), not a public issue.
