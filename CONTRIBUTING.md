# Contributing to wishket-radar

Thank you for your interest in contributing to `wishket-radar`!

## Development Setup

Prerequisites:
- [Rust 1.75+](https://rustup.rs/) (Cargo)

```bash
# Clone repository
git clone https://github.com/epicsagas/wishket-radar.git
cd wishket-radar

# Run tests
cargo test --manifest-path server/Cargo.toml

# Check formatting & lints
cargo fmt --manifest-path server/Cargo.toml --check
cargo clippy --manifest-path server/Cargo.toml --all-targets -- -D warnings
```

## Commit Message Guidelines

This repository enforces [Conventional Commits 1.0.0](https://www.conventionalcommits.org/):

- `feat`: new features or capabilities
- `fix`: bug fixes
- `docs`: documentation updates
- `refactor`: code changes without behavior changes
- `test`: adding or updating tests
- `chore`: build tools, dependencies, or auxiliary tasks

## Submitting a Pull Request

1. Fork the repo and create your branch from `main`.
2. Ensure `cargo test --manifest-path server/Cargo.toml` passes.
3. Verify formatting and lints with `cargo fmt` and `cargo clippy`.
4. Keep PRs focused on a single concern.
5. Update `README.md` or skill definitions if command behavior changes.
