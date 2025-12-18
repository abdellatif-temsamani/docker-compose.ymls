> NOTICE: containers are not production

# Table of Contents
- [Docker Manager](#docker-manager)
- [Available Containers](#available-containers)
- [Contributing](#contributing)
- [init.tmux - Automated Setup](#inittmux---automated-setup)
- [run.sh Usage](#runsh-usage)

# Docker Compose Manager

This repository provides a collection of docker-compose.yml configurations for development purposes and a Rust-based terminal UI application for interactively managing Docker Compose services.

## Docker Manager

The Docker Manager is a Rust-based terminal user interface (TUI) application for interactively managing Docker Compose services.

### Features
- Interactive selection and management of Docker Compose services
- Start, stop, and monitor service statuses
- Designed to work with the container configurations in this repository

### Dependencies
- Rust and Cargo (for building)
- Docker and Docker Compose

### Building and Running
1. Ensure Rust is installed: https://rustup.rs/
2. Build the application:
   ```bash
   cargo build --release
   ```
3. Run the manager:
   ```bash
   ./target/release/docker-manager
   ```

**Alternative:** If Rust or Cargo is not available, use the `run.sh` script for interactive container management.

### Usage
The application displays the app name and version in the bottom right corner.

**Navigation:**
- `Tab` / `Shift+Tab`: Navigate between Services and Logs panes
- `h`: Focus Services pane
- `l`: Focus Logs pane

**Services Pane:**
- `j` / `k`: Scroll through services
- `Space`: Toggle start/stop selected service
- `S`: Start selected service
- `s`: Stop selected service
- `/`: Search services (type to filter, Esc to exit)

**Logs Pane:**
- `j` / `k` / `Page Up` / `Page Down`: Scroll logs
- `Space`: Toggle auto-scroll
- `t`: Switch to Events tab
- `T`: Switch to Live Logs tab

**General:**
- `r`: Refresh services status
- `d`: Open Docker daemon control menu
- `q`: Quit

Keybinds are configurable in `keybinds.toml`.

Services are loaded from the `containers/` directory.

## Available Containers

Each directory contains a `docker-compose.yml` for its service:

- **mysql** - MySQL database with integrated phpMyAdmin
- **postgres** - PostgreSQL database with integrated Adminer
- **redis** - Redis cache
- **phpmyadmin** - phpMyAdmin standalone (for external MySQL databases)
- **adminer** - Adminer standalone (for external databases)

## Contributing

### Adding New Containers

To add a new container configuration:

1. Create a new directory under `containers/` (e.g., `containers/myapp/`)

2. Add a `docker-compose.yml` file with your service configuration. Follow these guidelines:
   - Use relative paths for volumes if needed
   - Expose ports appropriately for development
   - Include health checks where possible
   - Add comments for complex configurations

3. Test your configuration:
   ```bash
   cd containers/myapp
   docker compose up
   docker compose down
   ```

4. Update this README:
   - Add your container to the "Available Containers" list above
   - Describe what the container provides

5. Submit a pull request with your changes

### Guidelines
- Ensure containers are suitable for development environments
- Include necessary environment variables or configuration files
- Document any special setup requirements
- Follow Docker Compose best practices

## init.tmux - Automated Setup

The `init.tmux` file provides automated tmux session setup:

**What it does:**
- Automatically runs `run.sh` in tmux window 1
- Sets a session-closed hook that kills all running Docker containers when you exit tmux

**Usage:**

Start tmux and source the init file in one command:
```bash
tmux new -s containers \; source-file ./init.tmux
```

Or source it in an existing tmux session:
```bash
tmux source-file ./init.tmux
```

**Note:** The cleanup hook will stop all Docker containers (not just this project's) when the tmux session closes. If you have other containers running, use the manual method instead.

## run.sh Usage

The `run.sh` script provides an interactive way to start multiple containers.

### What it does
1. Runs `git pull` to update the repository
2. Ensures Docker service is running (via systemd)
3. Uses `fd` and `fzf` to let you select containers interactively
4. Starts each selected container in a new tmux window (or directly if not in tmux)

### Dependencies
- **docker** and **docker-compose**
- **fd** - fast file finder
- **fzf** - fuzzy finder for interactive selection
- **tmux** - terminal multiplexer (required for multi-window experience)
- **systemd** - for Docker service management (requires sudo)

### How to use
1. Make the script executable:
   ```bash
   chmod +x ./run.sh
   ```

2. Start a tmux session (recommended):
   ```bash
   tmux new -s containers
   ```

3. Run the script:
   ```bash
   ./run.sh
   ```

4. Select containers:
   - Use arrow keys to navigate
   - Press `Tab` to select multiple containers
   - Press `Enter` to start selected containers

5. Each container will open in its own tmux window

### Stopping containers
- In tmux: Press `Ctrl+C` in the window running the container
- From another terminal: `docker compose -f <dir>/docker-compose.yml down`
