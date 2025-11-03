#!/bin/sh

git pull

if ! systemctl is-active --quiet docker; then
    echo "Starting Docker service..."
    sudo systemctl start docker
else
    echo "Docker service is already running."
fi

containers=$(fd . . -t d | fzf -m)

[ -z "$containers" ] && echo "No container selected." && exit 1

for container in $containers; do
    name=$(basename "$container")
    echo "Creating tmux window for: $name"

    if [ -n "$TMUX" ]; then
        tmux new-window -n "$name" "cd '$container' && docker-compose up" \; select-window -t "$name"
    else
        echo "Not in tmux — running directly for $name..."
        (cd "$container" && docker-compose up)
    fi
done

