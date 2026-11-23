#!/bin/sh
# Records the catalog preview, asciiverse/icewire.cast: 100x30,
# `icewire --demo` (the league, night 4's scene, Sleet on the bench, the
# battle in the boiler room with a stretch on the live bench, the scene
# after, then night 5, the Long Window under Jupiter), about 53 seconds;
# the tour quits by itself.
# Runs asciinema in a detached tmux pane, so nothing from this terminal gets
# in; a throwaway HOME and state folder keep the session log out of the way
# (the demo saves nothing).
#
#   cargo build --release && asciiverse/record-preview.sh
set -eu
cd "$(dirname "$0")/.."
out=asciiverse/icewire.cast
tmp=$(mktemp -d)
trap 'tmux -L iw-rec kill-server 2>/dev/null || true; rm -rf "$tmp"' EXIT
rm -f "$out"
tmux -L iw-rec new-session -d -s rec -x 100 -y 30 \
  "env -i PATH=$PWD/target/release:/usr/bin:/bin:/snap/bin HOME=$tmp XDG_STATE_HOME=$tmp/state USER=player LOGNAME=player \
     TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8 \
   asciinema rec --window-size 100x30 --idle-time-limit 2 \
     --capture-env TERM,COLORTERM,LANG --title 'Icewire' \
     --command 'icewire --demo' $out"
sleep 58
ls -la "$out"
