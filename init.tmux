send-keys -t :1 'sh run.sh' C-m

# Automatically run `docker kill $(docker ps -q)` before any session is killed
set-hook -g session-closed 'run-shell "docker kill $(docker ps -q) 2>/dev/null || true"'

# vim: ft=tmux
