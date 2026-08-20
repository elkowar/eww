#!/bin/bash
# Frutiger Aero Weather — fetches weather from wttr.in (auto-location)
set -euo pipefail

FORCE=false
[ "${1:-}" = "--force" ] && FORCE=true

CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/eww-weather"
CACHE_FILE="$CACHE_DIR/weather.json"
CACHE_TTL=1800
ICON_DIR="./assets/icons"

mkdir -p "$CACHE_DIR"

if [ "$FORCE" = false ] && [ -f "$CACHE_FILE" ]; then
  cache_age=$(($(date +%s) - $(stat -c %Y "$CACHE_FILE" 2>/dev/null || echo 0)))
  if [ "$cache_age" -lt "$CACHE_TTL" ]; then
    cat "$CACHE_FILE"
    exit 0
  fi
fi

RESPONSE=$(curl -sf --max-time 10 "wttr.in?format=j1" 2>/dev/null) || {
  if [ -f "$CACHE_FILE" ]; then
    cat "$CACHE_FILE"
    exit 0
  fi
  echo '{"error":"no data"}'
  exit 1
}

echo "$RESPONSE" | python3 -c "
import json, sys
from datetime import datetime

d = json.load(sys.stdin)
cc = d['current_condition'][0]
area = d['nearest_area'][0]
city = area['areaName'][0]['value']
forecast = d['weather'][:3]

ICON_DIR = '$ICON_DIR'

def icon_name(code):
    c = int(code)
    if c == 113: return 'sunny'
    if c in (116, 119): return 'partly-cloudy'
    if c == 122: return 'cloudy'
    if c in (143, 248, 260): return 'foggy'
    if c in (176, 185, 263, 266, 281, 284, 293, 296, 299, 302, 305, 308): return 'rainy'
    if c in (311, 314, 317, 320, 323, 326, 329, 332, 335, 338): return 'snowy'
    return 'cloudy'

def day_name(date_str):
    d = datetime.strptime(date_str, '%Y-%m-%d')
    today = datetime.now().replace(hour=0, minute=0, second=0, microsecond=0)
    diff = (d - today).days
    if diff == 0: return 'Today'
    if diff == 1: return 'Tomorrow'
    names = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
    return names[d.weekday()]

now = datetime.now()
updated_str = now.strftime('%H:%M')

result = {
    'city': city,
    'temp': cc['temp_C'],
    'feels': cc['FeelsLikeC'],
    'condition': cc['weatherDesc'][0]['value'].strip(),
    'icon': f'{ICON_DIR}/{icon_name(cc[\"weatherCode\"])}.svg',
    'humidity': cc['humidity'],
    'wind': cc['windspeedKmph'],
    'uv': cc['uvIndex'],
    'cloud': cc['cloudcover'],
    'today_max': forecast[0]['maxtempC'],
    'today_min': forecast[0]['mintempC'],
    'updated_time': updated_str,
    'fc0_icon': f'{ICON_DIR}/{icon_name(forecast[0][\"hourly\"][0][\"weatherCode\"])}.svg',
    'fc0_temp': forecast[0]['maxtempC'],
    'fc0_min': forecast[0]['mintempC'],
    'fc0_day': day_name(forecast[0]['date']),
    'fc1_icon': f'{ICON_DIR}/{icon_name(forecast[1][\"hourly\"][0][\"weatherCode\"])}.svg',
    'fc1_temp': forecast[1]['maxtempC'],
    'fc1_min': forecast[1]['mintempC'],
    'fc1_day': day_name(forecast[1]['date']),
    'fc2_icon': f'{ICON_DIR}/{icon_name(forecast[2][\"hourly\"][0][\"weatherCode\"])}.svg',
    'fc2_temp': forecast[2]['maxtempC'],
    'fc2_min': forecast[2]['mintempC'],
    'fc2_day': day_name(forecast[2]['date']),
}
print(json.dumps(result))
" > "$CACHE_FILE.tmp" && mv "$CACHE_FILE.tmp" "$CACHE_FILE"

cat "$CACHE_FILE"
