# CLAUDE.md — commit-viz project instructions

## Project overview

commit-viz collects git repository data (branches, commits, merges, stats) via a **Python collector** and renders animated timeline visualizations via a **Rust renderer**. The pipeline: Python → JSON → Rust → MP4/PNG.

## Build & run

```bash
make build-collector    # uv sync in collector/
make build-renderer     # cargo build --release in renderer/
make lint               # ruff + clippy + mypy
make fmt                # ruff format + cargo fmt
make test               # pytest + cargo test
make coverage           # pytest-cov + cargo-llvm-cov
make rerun SLUG=<name>  # full pipeline for a repo
```

## Architecture

- `collector/src/commit_viz/` — Python 3.11+ data collection
- `renderer/src/` — Rust video/image rendering
- `analysis/<slug>/` — per-repo output (JSON, MP4, PNG)
- `schema/` — JSON schema for the data format

---

## Python conventions (collector)

### Style & tooling

- **Python 3.11+** — use modern syntax: `X | Y` unions, `match` statements, `str | None` (not `Optional[str]`), `list[str]` (not `List[str]`)
- **Formatter**: `ruff format` (line length 100)
- **Linter**: `ruff check` with pyflakes, pycodestyle, isort, bugbear, simplify, pyupgrade, pathlib, comprehensions
- **Type checker**: `mypy --check-untyped-defs`
- **Tests**: `pytest` with `pytest-cov`

### Code principles

- Prefer `dataclass` or `NamedTuple` for structured data over raw dicts
- Use type hints on all function signatures — parameters and return types
- Use `pathlib.Path` over `os.path` for filesystem operations
- Use f-strings over `.format()` or `%`-formatting
- Prefer list/dict/set comprehensions over manual loops when readable
- Use `itertools`, `collections`, and standard library before reaching for deps
- Use structural pattern matching (`match`/`case`) when branching on type or shape
- Prefer early returns over deeply nested if/else
- Keep functions focused — extract helpers when a function exceeds ~30 lines
- Use `from __future__ import annotations` for forward references (already in codebase)
- Sort imports: stdlib → third-party → local (enforced by ruff isort)
- Exception handling: catch specific exceptions, never bare `except:`
- Use context managers (`with`) for resource management

### Naming

- `snake_case` for functions, variables, modules
- `PascalCase` for classes
- `UPPER_SNAKE_CASE` for module-level constants
- Leading underscore `_name` for internal/private
- Descriptive names — avoid single-letter vars except in comprehensions and lambdas

### Testing

- Test files in `collector/tests/`, named `test_*.py`
- Use `pytest` fixtures, parametrize for variant testing
- Aim for coverage of core logic (models, git collection, stats)

---

## Rust conventions (renderer)

### Style & tooling

- **Edition 2021**, targets stable Rust
- **Formatter**: `cargo fmt` (rustfmt, max_width=100)
- **Linter**: `cargo clippy` with pedantic warnings enabled
- **Tests**: `cargo test`
- **Coverage**: `cargo llvm-cov` (when available)

### Code principles

- Prefer borrowing (`&T`, `&str`) over cloning — minimize `.clone()` calls
- Use iterators and combinators (`.map()`, `.filter()`, `.collect()`) over index loops
- Use `?` operator for error propagation — avoid `.unwrap()` except in tests
- Prefer `if let` / `match` for Option/Result handling
- Use `HashMap`/`HashSet` imports at top of file, not inline `std::collections::HashMap`
- Create type aliases for complex types: `type BranchMap<'a> = HashMap<&'a str, f32>;`
- Keep `fn main()` minimal — delegate to library functions
- Structure: one concept per module, public API via `pub` functions/structs
- Use `#[derive(Debug, Clone)]` generously on data structs
- Prefer `&str` parameters over `String` when ownership isn't needed
- Use `const` for compile-time constants, not `let`

### Error handling

- Use `Result<T, Box<dyn std::error::Error>>` for app-level errors
- Use `thiserror` for library-level custom error types if needed
- Provide context in error messages
- Never panic in library code — reserve `unwrap()` for tests and provably-safe cases

### Naming

- `snake_case` for functions, variables, modules
- `PascalCase` for types, traits, enum variants
- `SCREAMING_SNAKE_CASE` for constants
- Descriptive names — `positioned_commits` not `pcs`

### Performance

- Use `rayon` for CPU-bound parallel work (already in use)
- Avoid unnecessary allocations in hot paths (frame rendering)
- Prefer `Vec::with_capacity()` when size is known
- Use `&[T]` slices over `&Vec<T>` in function signatures

### Testing

- Unit tests in `#[cfg(test)] mod tests` within each module
- Integration tests in `renderer/tests/`
- Test data loading, layout math, and edge cases

---

## Commit style

- Imperative mood: "Add feature" not "Added feature"
- Reference issue numbers when applicable
- Keep first line under 72 chars
