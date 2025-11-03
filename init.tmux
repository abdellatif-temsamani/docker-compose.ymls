# Run the script inside window 1
send-keys -t :1 'sh run.sh' C-m

# Attach hook to *this* session only
set-hook -t "$(tmux display-message -p '#S')" session-closed 'run-shell "docker kill \\$(docker ps -q) 2>/dev/null || true"'
# vim: ft=tmux
