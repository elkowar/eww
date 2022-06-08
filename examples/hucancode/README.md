# Screenshots
// TBD
# Installation
Install required fonts (Iosevka Nerd Font & Beyno)
```
cp fonts/* ~/.local/share/fonts
```
Install required packages
```
# for music
mpd alsa-utils
# for workspace indicator
xdotool xprop 
# for screenshot tool
xclip maim imagemagick 
```
Additionally, for laucher and lock button to work, you need
```
rofi i3lock
```
Bind those command to your favorite key, here's mine
```
# on login
eww open bar
# super + x
eww open --toggle powermenu
# super + s
eww open --toggle takeshot
```
# Customization
Weather section is now pointing to my city, edit `wttr` section to match yours.
By default log out command only support `openbox` and `bspwm`. You should change it accordingly.
I use `rofi` and `i3lock`, but you might differ. Change those command to suit your case.
