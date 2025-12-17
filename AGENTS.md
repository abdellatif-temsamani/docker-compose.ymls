# Agent Guidelines for docker-compose.ymls

## Build/Lint/Test Commands
- **Build**: `cargo build --release`
- **Type check**: `cargo check`
- **Lint**: `cargo clippy`
- **Test all**: `cargo test`
- **Test single**: `cargo test <test_name>`
- **Format**: `cargo fmt`

## Code Style Guidelines
- **Language**: Rust 2024 edition, modern idiomatic patterns
- **Naming**: snake_case (functions/variables), PascalCase (types/structs), SCREAMING_SNAKE_CASE (constants)
- **Imports**: std → external crates → local modules (group by functionality)
- **Error Handling**: Result/Option types, `?` operator, `.unwrap_or(false)` for graceful degradation
- **Patterns**: `Command::new()`, `match` expressions, iterator chains, `if let`, `saturating_sub()`
- **TUI**: ratatui with Blue borders, Green=running, Red=error, Yellow=transitioning
- **Organization**: Single-purpose functions, structs for state, enums for status, separate UI/business logic

### No Cursor/Copilot Rules Found
No .cursor/rules/ or .github/copilot-instructions.md files present.