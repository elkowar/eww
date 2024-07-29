#!/bin/sh

get_song() {
  song=`mpc -f "[[%artist% - ]%title%]|[%file%]" current`
  echo ${song:-"Offline"}
}

get_status() {
  mpc | awk 'NR==2' | grep -q 'playing' && echo '' || echo ''
}

if [[ "$1" == "song" ]]; then
  get_song
  while true; do
    mpc idle player >/dev/null && get_song
  done
elif [[ "$1" == "status" ]]; then
  get_status
  while true; do
    mpc idle player >/dev/null && get_status
  done
fi
