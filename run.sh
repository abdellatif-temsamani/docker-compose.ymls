#!/bin/sh

git pull

if ! systemctl is-active --quiet docker; then
    echo "Starting Docker service..."
    sudo systemctl start docker
else
    echo "Docker service is already running."
fi

container=$(fd . . -t d | fzf)

cd "$container" || echo "error on cd to $container" exit 2
docker-compose up
