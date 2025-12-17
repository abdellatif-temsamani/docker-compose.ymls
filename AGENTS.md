# Agent Guidelines for docker-compose.ymls

## Build/Lint/Test Commands

- **Build**: `cargo build --release`
- **Type check**: `cargo check`
- **Lint**: `cargo clippy`
- **Test all**: `cargo test`
- **Test single**: `cargo test <test_name>` (no tests currently exist)
- **Format**: `cargo fmt` (uses default rustfmt settings)

## Code Style Guidelines

### Language & Edition
- Rust 2024 edition
- Target modern, idiomatic Rust patterns

### Naming Conventions
- Functions/variables: snake_case
- Types/structs: PascalCase
- Constants: SCREAMING_SNAKE_CASE
- Modules: snake_case

### Imports
- Standard library imports first
- External crates second
- Local modules last
- Group by functionality

### Error Handling
- Use `Result<T, E>` types for fallible operations
- Use `Option<T>` for optional values
- Prefer `?` operator for error propagation
- Use `.unwrap_or(false)` for graceful degradation

### Patterns & Idioms
- Use `Command::new()` for system operations
- Use `match` expressions for comprehensive pattern matching
- Use iterator chains (`.filter().map().collect()`)
- Use closures where appropriate
- Use `if let` for optional binding
- Use `saturating_sub()` for safe arithmetic

### TUI/Graphics
- Use ratatui for terminal UI components
- Consistent color scheme: Blue borders, Green=running, Red=stopped/error, Yellow=transitioning
- Use `Style::default().fg(Color::X)` pattern
- Use `Block::default().borders(Borders::ALL)` pattern

### Code Organization
- Keep functions focused and single-purpose
- Use structs for complex state
- Use enums for status/service states
- Separate UI logic from business logic

### No Cursor/Copilot Rules Found
No .cursor/rules/ or .github/copilot-instructions.md files present.