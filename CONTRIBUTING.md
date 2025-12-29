# Contributing to Ormada

Thank you for your interest in contributing to Ormada! This document provides guidelines for contributing.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/ormada.git`
3. Create a branch: `git checkout -b feature/your-feature-name`

## Development Setup

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace

# Run benchmarks
cargo bench
```

## Code Style

- Follow Rust idioms and best practices
- Run `cargo fmt` before committing
- Ensure `cargo clippy --workspace` passes without warnings
- Write tests for new functionality
- Document public APIs with rustdoc comments

## Pull Request Process

1. Ensure all tests pass: `cargo test --workspace`
2. Ensure clippy is clean: `cargo clippy --workspace`
3. Update documentation if needed
4. Write a clear PR description explaining your changes
5. Link any related issues

## Commit Messages

Use clear, descriptive commit messages:

- `feat: add soft delete support`
- `fix: resolve N+1 query in prefetch_related`
- `docs: update README with migration examples`
- `refactor: simplify QuerySet internals`
- `test: add integration tests for transactions`

## Reporting Issues

When reporting issues, please include:

- Rust version (`rustc --version`)
- Ormada version
- Database backend (PostgreSQL, SQLite, MySQL)
- Minimal reproduction code
- Expected vs actual behavior

## Code of Conduct

Be respectful and constructive. We're all here to build great software together.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
