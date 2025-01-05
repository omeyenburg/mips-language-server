#!/bin/sh

session="mips-ls"

# Check if the session already exists
tmux has-session -t $session 2>/dev/null

# $? is the exit status of the last command
if [ $? != 0 ]; then
    tmux new-session -d -s $session

    tmux new-window -t $session:1
    tmux send-keys -t $session:1 'nix develop' C-m

    tmux new-window -t $session:2

    tmux new-window -t $session:3
    tmux split-window -h
    tmux send-keys -t $session:3.1 'vim' C-m
    tmux send-keys -t $session:3.1 ':term tail -f lsp.log' C-m
    tmux send-keys -t $session:3.1 'G' C-m

    tmux select-pane -t $session:3.0
    tmux select-window -t $session:1
fi

# Attach to the tmux session
tmux attach -t $session
