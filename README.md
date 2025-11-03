> NOTICE: containers are not production

# Table of Content
- [Available Containers](#available-containers)
- [run.sh Usage](#runsh-usage)
- [init.tmux - Automated Setup](#inittmux---automated-setup)

# docker-compose.ymls

This repository aims to collect docker-compose.yml config, the collection
intended to for development purposes only.

## Available Containers

Each directory contains a `docker-compose.yml` for its service:

- **mysql** - MySQL database with phpMyAdmin
- **postgres** - PostgreSQL database with Adminer
- **redis** - Redis cache
- **phpmyadmin** - phpMyAdmin standalone

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

### init.tmux - Automated Setup

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
