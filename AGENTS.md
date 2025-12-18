# Agent Guidelines for docker-manager

## Build/Lint/Test Commands

- Build: `cargo build --release`
- Type check: `cargo check`
- Lint: `cargo clippy`
- Test all: `cargo test`
- Test single: `cargo test <test_name>`
- Format: `cargo fmt`
- Running: I run the app myself and test it

## Code Style Guidelines

- Language: Rust 2024, async with tokio
- Naming: snake_case (fn/var), PascalCase (type/struct), SCREAMING_SNAKE_CASE
  (const)
- Imports: std → external crates → local modules
- Error handling: Result/Option, `?`, `.unwrap_or(false)` graceful degradation
- Patterns: `Command::new()`, `match`, iterators, `if let`, `saturating_sub()`
- TUI: ratatui, Blue=focus, Green=running, Red=error, Yellow=transitioning
- Organization: Single-purpose fns, structs for state, enums for status,
  separate UI/business logic

## Keybinds (configurable in keybinds.toml)

- App: q=quit, /=search, d=daemon, r=refresh, h=focus services, l=focus logs,
  j/k=scroll
- Services: s=stop, S=start, space=toggle
- Logs: space=toggle auto-scroll, t=switch tabs
- Navigation: tab/shift-tab=next/prev service, up/down/pageup/pagedown=scroll

## Rules

No .cursor/rules/ or .github/copilot-instructions.md found.
