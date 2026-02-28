# Contributing to crgx

Thank you for your interest in contributing to crgx! This document provides guidelines and instructions for contributing.

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## How Can I Contribute?

### Reporting Bugs

Before creating bug reports, please check existing issues to avoid duplicates.

When creating a bug report, include:

- **Clear title** describing the issue
- **Steps to reproduce** the behavior
- **Expected behavior** vs what actually happened
- **Environment details**:
  - crgx version (`crgx --version`)
  - Operating system and architecture
  - Rust toolchain version (if relevant)
- **Error messages** (full output if available)

### Suggesting Features

Feature requests are welcome! Please include:

- **Clear description** of the feature
- **Use case** — why is this feature needed?
- **Proposed CLI interface** (if applicable)
- **Alternative solutions** you've considered

### Pull Requests

1. **Fork** the repository
2. **Create a branch** from `main`:
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. **Make your changes** following our style guidelines
4. **Add tests** for new functionality
5. **Run tests** to ensure nothing is broken:
   ```bash
   cargo test
   ```
6. **Commit** with a clear message (see commit guidelines below)
7. **Push** and create a Pull Request

## Development Setup

### Prerequisites

- Rust (stable, latest)
- Git

### Building from Source

```bash
git clone https://github.com/yfedoseev/crgx.git
cd crgx
cargo build
```

### Running Tests

```bash
cargo test
```

### Running the Dev Build

```bash
cargo run -- tokei .
```

## Style Guidelines

### Rust

- Follow standard Rust formatting (`cargo fmt`)
- Pass clippy checks (`cargo clippy -- -D warnings`)
- Use meaningful variable and function names
- Add doc comments for public APIs
- Use `thiserror` for error types, `anyhow` for error propagation

### Git Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code style (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Examples:**
```
feat(resolve): add quickinstall fallback source
fix(cache): handle corrupted metadata gracefully
docs(readme): update installation instructions
```

## Pull Request Checklist

- [ ] Code compiles without warnings (`cargo build`)
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt --check`)
- [ ] Clippy passes (`cargo clippy -- -D warnings`)
- [ ] Documentation updated (if applicable)
- [ ] Commit messages follow Conventional Commits

## Questions?

Feel free to:

- Open a [GitHub Issue](https://github.com/yfedoseev/crgx/issues) with the `question` label
- Start a [GitHub Discussion](https://github.com/yfedoseev/crgx/discussions)

## License

By contributing, you agree that your contributions will be licensed under the [MIT](LICENSE) license.
