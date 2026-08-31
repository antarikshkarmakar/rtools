# Contributing to rtools

Thank you for your interest in contributing to rtools! This document provides guidelines and information about contributing.

## Development Setup

### Prerequisites

- Rust 1.75 or later
- Cargo (comes with Rust)
- Git

### Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/yourusername/rtools.git
   cd rtools
   ```
3. Create a branch:
   ```bash
   git checkout -b feature/my-feature
   ```
4. Make your changes
5. Run tests:
   ```bash
   cargo test
   ```
6. Commit and push:
   ```bash
   git add .
   git commit -m "Add my feature"
   git push origin feature/my-feature
   ```
7. Create a Pull Request

## Code Style

- Follow Rust standard style guidelines
- Use `cargo fmt` to format code
- Use `cargo clippy` to check for linting issues
- Write documentation for public APIs

## Testing

- Write unit tests for new functionality
- Add integration tests for new features
- Ensure all tests pass before submitting PR

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p rtools-image

# Run benchmarks
cargo bench
```

## Commit Messages

- Use clear, descriptive commit messages
- Start with a verb in imperative mood
- Keep first line under 72 characters
- Reference issues when applicable

**Examples:**
- `Add WebP compression support`
- `Fix HEIC conversion bug`
- `Update API documentation`

## Pull Request Process

1. Update documentation if needed
2. Add tests for new functionality
3. Ensure CI passes
4. Request review from maintainers

## Code of Conduct

- Be respectful and inclusive
- Welcome newcomers
- Focus on constructive feedback
- Help others learn and grow

## Questions?

Open an issue or reach out to the maintainers.