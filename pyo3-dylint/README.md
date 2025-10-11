# pyo3-dylint

Custom lints for PyO3-specific patterns and best practices using [dylint](https://github.com/trailofbits/dylint).

## Overview

This crate provides domain-specific lints for PyO3 code using [dylint](https://github.com/trailofbits/dylint). These lints help catch PyO3-specific antipatterns that can lead to bugs, deadlocks, or unsafe behavior when interfacing with Python.

## Available Lints

### `mutex_lock_py_attached`

**Level:** Warn

Detects direct calls to `.lock()` on `Mutex` types when a `Python<'_>` token is available in scope. This pattern can cause deadlocks because the thread holds attachment to the Python runtime while waiting for the lock.

**Bad:**
```rust
use pyo3::prelude::*;
use std::sync::Mutex;

fn example(py: Python<'_>, mutex: &Mutex<i32>) {
    let guard = mutex.lock().unwrap(); // ⚠️ Could deadlock!
}
```

**Good:**
```rust
use pyo3::prelude::*;
use pyo3::sync::MutexExt;
use std::sync::Mutex;

fn example(py: Python<'_>, mutex: &Mutex<i32>) {
    let guard = mutex.lock_py_attached(py).unwrap(); // ✓ Safe!
}
```

## Usage

### Prerequisites

Install `cargo-dylint` and `dylint-link`:

```bash
cargo install cargo-dylint dylint-link
```

### Running the Lints

From the PyO3 repository root:

```bash
cargo dylint --all --workspace -- --manifest-path path/to/your/Cargo.toml
```

Or run on the PyO3 codebase itself:

```bash
cargo dylint mutex_lock_py_attached --path pyo3-dylint --workspace
```

### Using in Your Project

Add the following to your project's `Cargo.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
    { git = "https://github.com/PyO3/pyo3", pattern = "pyo3-dylint" }
]
```

Then run:

```bash
cargo dylint --all --workspace
```

## Development

### Building

```bash
cd pyo3-dylint
cargo build
```

### Testing

Run the UI tests:

```bash
cargo test
```

### Adding New Lints

1. Create a new module in `src/` for your lint
2. Implement `LateLintPass` or `EarlyLintPass`
3. Register the lint in `src/lib.rs`
4. Add UI test cases in `ui/`
5. Update this README with lint documentation

## Architecture

- **`src/lib.rs`**: Main entry point, registers all lints
- **`src/mutex_lock_py_attached.rs`**: Example lint implementation
- **`ui/`**: UI test cases that verify lint behavior

## References

- [Dylint Documentation](https://github.com/trailofbits/dylint)
- [Clippy Lint Development](https://doc.rust-lang.org/nightly/clippy/development/adding_lints.html)
- [PyO3 Thread Safety Guide](https://pyo3.rs/latest/class/thread-safety.html)
