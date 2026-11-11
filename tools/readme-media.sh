#!/bin/sh
# Renders the README's media into assets/readme with the app's headless renderer:
#   sunset.png  one block: early, late, and the bell with its lantern
#   bell.gif    the last seconds of a block, the bell, and the lantern it lights
#   night.gif   a long break: the four lanterns rise into the night
#   places.png  Coast, Lake and Mesa late in a block, light over dark
#   today.png   Today and the four weeks behind it, light beside dark
# Run after `cargo build --release`. Needs ffmpeg.
set -e
cd "$(dirname "$0")/.."
APP=target/release/pomodoro
OUT=assets/readme
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$OUT"

# A GIF of `from` to `to` seconds of a video at `speed`. One palette for every frame keeps
# frames small, so each GIF stays within one stretch of sky; a whole sunset would band. The last
# frame holds for HOLD seconds, then dissolves over FADE into the first, so the loop never jumps.
HOLD=0.8
FADE=0.6
gif() {
  at=$(awk "BEGIN { print ($4 - $3) / $5 + $HOLD - $FADE }")
  ffmpeg -loglevel error -y -i "$1" -filter_complex \
    "trim=$3:$4,setpts=(PTS-STARTPTS)/$5,fps=15,scale=380:-1:flags=lanczos,split[run][head];\
[run]tpad=stop_mode=clone:stop_duration=$HOLD[held];\
[head]trim=end_frame=1,setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration=$FADE[first];\
[held][first]xfade=transition=fade:duration=$FADE:offset=$at,split[a][b];\
[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
    "$2"
}

for sc in focus-early focus-late; do
  $APP --scene $sc --theme light --snap "$TMP/$sc.png" --at 0.6 >/dev/null 2>&1
done
# The bell at the lantern's bloom.
$APP --scene bell --theme light --snap "$TMP/bell.png" --at 2.0 >/dev/null 2>&1
ffmpeg -loglevel error -y -i "$TMP/focus-early.png" -i "$TMP/focus-late.png" -i "$TMP/bell.png" \
  -filter_complex "hstack=3,scale=iw/2:-1:flags=lanczos" "$OUT/sunset.png"

$APP --scene last-minute --script scripts/bell.json --theme light --video "$TMP/bell.mp4" --fps 30 --scale 2 >/dev/null 2>&1
gif "$TMP/bell.mp4" "$OUT/bell.gif" 55 66 1

$APP --scene night-minute --script scripts/night.json --video "$TMP/night.mp4" --fps 30 --scale 2 >/dev/null 2>&1
gif "$TMP/night.mp4" "$OUT/night.gif" 0.4 31 3

# Each place three scenes in: `t` steps to the next place.
for n in 0 1 2; do
  python3 -c "import json; print(json.dumps({'events': [{'t': 0.05 + 0.02 * i, 'type': 'key', 'name': 't'} for i in range($n)]}))" > "$TMP/place$n.json"
  for theme in light dark; do
    $APP --script "$TMP/place$n.json" --scene focus-late --theme $theme --snap "$TMP/place$n-$theme.png" --at 0.6 >/dev/null 2>&1
  done
done
ffmpeg -loglevel error -y \
  -i "$TMP/place0-light.png" -i "$TMP/place1-light.png" -i "$TMP/place2-light.png" \
  -i "$TMP/place0-dark.png" -i "$TMP/place1-dark.png" -i "$TMP/place2-dark.png" \
  -filter_complex "[0][1][2]hstack=3[l];[3][4][5]hstack=3[d];[l][d]vstack=2,scale=iw/2:-1:flags=lanczos" "$OUT/places.png"

for theme in light dark; do
  $APP --scene today --theme $theme --snap "$TMP/today-$theme.png" --at 0.6 >/dev/null 2>&1
done
ffmpeg -loglevel error -y -i "$TMP/today-light.png" -i "$TMP/today-dark.png" -filter_complex hstack=2 "$OUT/today.png"

ls -l "$OUT"
