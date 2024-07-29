#!/bin/sh

print_workspaces() {
    buf=""
    n=$(xdotool get_num_desktops)
    focused=$(xdotool get_desktop)
    for (( i=0; i<$n; i++ )) do
        if [ "$focused" == "$i" ]; then
            icon="●"
            class="focused"
        elif xdotool search --desktop $i --limit 1 "" >/dev/null; then
            icon="◉"
            class="occupied"
        else
            icon="○"
            class="empty"
        fi 
        buf="$buf (eventbox :cursor \"hand\" (button :class \"$class\" :onclick \"xdotool set_desktop $i\" \"$icon\"))"
    done
    echo "(box :class \"workspaces\" :spacing 10 :halign \"center\" :valign \"center\" :vexpand true $buf)"
}

print_workspaces
xprop -spy -root _NET_CURRENT_DESKTOP | while read -r _ ; do
    print_workspaces
done 
