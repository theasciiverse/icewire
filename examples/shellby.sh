#!/bin/sh
# Shellby: an Icewire bot in plain POSIX shell, with nothing but builtins.
#
# It turns toward the compass beacon (the next checkpoint in a race, the
# other rover in a battle), keeps driving, and fires when it is nearly
# aimed. The shell has no fractions, so it thinks in whole degrees.
#
# Try it:   icewire --tournament sh examples/shellby.sh sprocket
# The lines it reads and writes are in PROTOCOL.md.

# Icewire says hello, names the rover and the challenge, then "end".
read hello
while read line; do
  [ "$line" = end ] && break
done
echo "icewire-bot 1"

bearing=0
while read key a b c d rest; do
  case "$key" in
    compass)
      # compass heading H bearing B distance D
      bearing=${d%.*}
      [ -z "$bearing" ] || [ "$bearing" = "-" ] && bearing=0
      ;;
    end)
      if [ "$bearing" -gt 5 ]; then
        echo "gyro 0.6"
      elif [ "$bearing" -lt -5 ]; then
        echo "gyro -0.6"
      else
        echo "gyro 0"
      fi
      echo "tracks 0.7 0.7"
      if [ "$bearing" -gt -8 ] && [ "$bearing" -lt 8 ]; then
        echo "fire"
      fi
      echo "end"
      ;;
  esac
done
