# pushkind-crawlers

This README is intentionally minimal.

## Canonical Documentation

- [`SPEC.md`](SPEC.md): system behavior and execution contract (runtime,
  messaging, crawling, repository, benchmark pipeline).
- [`AGENTS.md`](AGENTS.md): contributor and code-generation rules (coding
  standards, architecture boundaries, testing expectations).

## Quick Pointers

- Main entrypoint: `src/main.rs`
- Service configuration: `config/default.yaml` (+ `config/local.yaml`)
- Manual message sender: `test_client.py`
- Example systemd unit: `pushkind-crawlers.service`

## Smoke Test

1. Start the service:
```bash
cargo run --release
```
2. In another terminal, send a sample message:
```bash
python test_client.py
```

The service logs when crawlers and benchmarks start/finish, and any issues with
fetching, parsing, or database updates.

## Development workflow

- Format: `cargo fmt --all`
- Lint: `cargo clippy --all-features --tests -- -Dwarnings`
- Tests: `cargo test --all-features`
- One-shot helper: `make check`
- Source of truth: `SPEC.md`
- Feature specs: `specs/features/<name>.md`
- Implementation plans: `plans/<name>.md`
- Architecture decisions (ADRs): `specs/decisions/`

When adding new functionality, follow the guidelines in [`AGENTS.md`](AGENTS.md)
to keep code consistent with the existing architecture.
