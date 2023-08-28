#!/bin/sh

if command -v pamixer &>/dev/null; then
    if [ true == $(pamixer --get-mute) ]; then
        echo 0
        exit
    else
        pamixer --get-volume
    fi
else
    amixer -D pulse sget Master | awk -F '[^0-9]+' '/Left:/{print $3}'
fi
