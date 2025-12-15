#!/bin/bash

if ! systemctl is-active --quiet docker; then
    echo "Starting Docker service..."
    sudo systemctl start docker
else
    echo "Docker service is already running."
fi

containers=$(fd . . -t d | fzf -m)

[ "$containers" = "" ] && echo "No container selected." && exit 1

for container in "${containers[@]}"; do
    name=$(basename "$container")

    if cd "$container" && docker-compose ps -q | grep -q .; then
        echo "Container '$name' is already running."
        cd - >/dev/null
        continue
    fi
    cd - >/dev/null

    echo "Creating tmux window for: $name"

    if [ "$TMUX" != "" ]; then
        tmux new-window -n "$name" "cd '$container' && docker-compose up" \; select-window -t "$name"
        tmux select-window -t :1
    else
        echo "Not in tmux — running directly for $name..."
        (cd "$container" && docker-compose up)
    fi
done
