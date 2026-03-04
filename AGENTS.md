# AGENTS.md

Guidelines for AI coding agents working in the `renderling.xyz` repository.

## Project Overview

Rust static site generator for [renderling.xyz](https://renderling.xyz). Cargo workspace
with two crates:

- **`rxyz`** (`crates/rxyz/`) -- Library. Markdown-to-HTML rendering via mogwai SSR,
  syntax highlighting (syntect/Dracula), RSS/Atom feed generation.
- **`xtask`** (`crates/xtask/`) -- Binary. CLI for building the site, cloning the
  renderling repo for docs/manual, and deploying to AWS S3/CloudFront via `pusha`.

External dependency: `pusha` crate at `../../../../pusha` (local path).

## Build / Lint / Test Commands

```bash
# Build
cargo build                       # entire workspace
cargo build -p rxyz               # library only
cargo build -p xtask              # CLI only

# Lint (no custom clippy.toml -- uses defaults)
cargo clippy --workspace
cargo clippy -p rxyz

# Format (no custom rustfmt.toml -- uses defaults)
cargo fmt --check                 # check only
cargo fmt                         # apply formatting

# Test
cargo test                        # all tests in workspace
cargo test -p rxyz                # tests in rxyz crate only
cargo test -p rxyz test_name      # run a single test by name
cargo test -p rxyz test_name -- --nocapture   # with stdout

# Site generation (requires pusha crate and content/ directory)
cargo xtask build                 # build site to site/
cargo xtask --renderling-refresh -e local build   # with doc refresh
```

`cargo xtask` is an alias defined in `.cargo/config.toml` for `cargo run --package xtask --`.

### Development workflow

```bash
cargo watch -x 'xtask build'     # rebuild on changes
basic-http-server site            # serve locally
```

### Deployment

```bash
cargo xtask --renderling-refresh -e staging deploy
cargo xtask --renderling-refresh -e production deploy
```

## Code Style

### Imports

Order: `std` first, blank line, external crates, blank line, local (`crate::`/`super::`).

```rust
use std::collections::BTreeMap;

use chrono::NaiveDate;
use mogwai::prelude::*;
use snafu::prelude::*;

use crate::md;
```

- Glob imports are acceptable for prelude modules (`mogwai::prelude::*`, `snafu::prelude::*`).
- Import specific items from large modules rather than globs:
  ```rust
  use markdown::mdast::{Code, Definition, Heading, Html, Image, Link, List, Text};
  ```
- Function-scoped imports are fine when a dependency is only used in one place (e.g.,
  `syntect` imports inside a single match arm in `md.rs`).

### Naming

Standard Rust conventions -- no project-specific overrides:

| Kind              | Convention           | Example                          |
|-------------------|----------------------|----------------------------------|
| Functions/methods | `snake_case`         | `render_markdown_page`           |
| Variables         | `snake_case`         | `url_root`, `last_build_date`    |
| Types/structs     | `PascalCase`         | `Site`, `AstRenderer`, `FeedItem`|
| Enum variants     | `PascalCase`         | `Error::InvalidUri`              |
| Constants         | `SCREAMING_SNAKE`    | `CSS`, `DRACULA_BYTES`           |
| Modules           | `snake_case`         | `md`, `feed`                     |

### Error Handling

The project uses **`snafu`** (not `anyhow` or `thiserror`) for structured errors in the
`rxyz` library crate.

```rust
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Yaml: {source}"))]
    Yaml { source: serde_yaml::Error },
    // ...
}

// Use context selectors:
serde_yaml::from_str(&input).context(YamlSnafu)?;

// Or construct manually when needed:
.map_err(|message| crate::Error::Md { message })?;
```

In the **`xtask`** CLI crate, `unwrap()` and `panic!()` are acceptable since it is a
build tool, not a library. Pattern: `log::error!()` then `panic!()` for fatal errors.

### Logging

Uses the `log` crate (not `tracing`). Initialized with `env_logger` in xtask.

- `trace!` -- verbose internal debugging (markdown node processing)
- `debug!` -- intermediate results
- `info!`  -- progress messages in xtask
- `warn!`  -- non-fatal issues (parse failures, missing optional data)
- `error!` -- fatal issues (followed by panic in xtask)

Use inline format args: `log::info!("found {count} items")`.

### Documentation

- `//!` module-level doc comments at the top of every module.
- `///` doc comments on all public functions, structs, and enum variants.
- Inline `//` comments for non-obvious logic.
- `todo!("description")` for known unimplemented features.

### Types and Generics

- Use `impl Trait` in function arguments: `fn new(url_root: impl AsRef<str>)`.
- The mogwai rendering code is generic over `V: View` -- maintain this pattern.
- Use path-qualified derives: `#[derive(serde::Serialize, serde::Deserialize)]`
  (don't import derive macros separately).
- Embed static assets with `include_str!` / `include_bytes!`.

### Tests

Tests live in-file as `#[cfg(test)] mod tests` blocks (no separate `tests/` directory).

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_news_date() {
        assert_eq!(
            parse_news_date("Wed 11 Feb, 2026"),
            Some(NaiveDate::from_ymd_opt(2026, 2, 11).unwrap())
        );
    }
}
```

- Name tests `test_<function_name>`.
- Use `assert_eq!` for value comparisons, `assert!(x.contains(...))` for substring checks.
- Construct test data inline (no fixture files).
- Use raw string literals (`r#"..."#`) for multi-line markdown/HTML test input.

### Async

The `xtask` crate uses `#[tokio::main]` with full features. Use `tokio::process::Command`
for subprocesses and `tokio::fs` for file I/O in async contexts.

### HTML Templating

The `rsx!` macro (from mogwai) is used for building HTML view trees with JSX-like syntax:

```rust
rsx! {
    let nav = nav {
        h1() {
            a(href = self.site_path("/")?) { "renderling" }
        }
    }
}
```

## Configuration Notes

- **No CI/CD pipeline** -- builds and deploys are manual via `cargo xtask`.
- **No rustfmt.toml or clippy.toml** -- all default settings apply.
- **No pinned rust-toolchain** -- uses whatever stable toolchain is installed.
- **Cargo.lock is gitignored** (workspace is treated as an application).
- **Workspace dependencies** are declared in root `Cargo.toml`: `clap`, `log`, `serde`,
  `serde_yaml`, `snafu`. Use `{workspace = true}` in crate `Cargo.toml` files.
