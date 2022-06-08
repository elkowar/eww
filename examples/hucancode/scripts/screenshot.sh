#!/bin/sh

screenshot=/tmp/screenshot.png
path=~/Pictures/Screenshots

window () {
    maim -st 9999999 | convert - \( +clone -background black -shadow 80x3+5+5 \) +swap -background none -layers merge +repage $screenshot
}

area () {
    maim -s $screenshot
}

screen () {
    # sleep 0.6;
    maim $screenshot
}

preview () {
    eww update screenshotpath=$screenshot
    eww open previewshot
}

discard () {
    rm "${screenshot}"
}

save () {
    name="screenshot-$(date +%h-%m-%s_%d-%m-%y).png"
    mkdir -p $path
    cp $screenshot $path/$name
    notify-send "screenshot saved!" -i $screenshot
}

copy () {
    xclip -selection clipboard -t image/png -i $screenshot
    notify-send "screenshot copied!" -i $screenshot
}

case $1 in
    "screen") screen && preview ;;
    "window") window && preview ;;
    "area") area && preview ;;
    "screen-quiet") screen && save && copy ;;
    "window-quiet") window && save && copy ;;
    "area-quiet") area && save && copy ;;
    "discard") discard ;;
    "copy") copy ;;
    "save") save ;;
    *) echo invalid option;;
esac
