#!/bin/sh
# Renders the review pack into shots/review: every scene in every place, light and dark; Today
# and History in each place, light and dark; the hover controls; a video of the last minute of a
# sunset through the bell into the break, and one of a one-minute long break through to dawn. Then frames.txt: per scene, the CPU time of its
# first frames (before the GPU queue fills and frame times measure the GPU instead), and how
# many frames in two seconds differ from the one before, which is what a window redraws.
# Run after `cargo build --release && python3 tools/scripts.py`.
set -e
cd "$(dirname "$0")/.."
APP=target/release/pomodoro
OUT=shots/review
TMP=$(mktemp -d)
rm -rf "$OUT" && mkdir -p "$OUT"
SCENES="wound focus-early focus-late paused bell bell-bloom skipped short long evening"

# A scene name, and when to take it: the bell twice, at the green flash and as the lantern blooms.
shot() {
  case $1 in
    wound) echo "-- 0.6" ;;
    bell) echo "bell 1.4" ;;
    bell-bloom) echo "bell 2.0" ;;
    skipped) echo "skipped 1.1" ;;
    *) echo "$1 0.6" ;;
  esac
}

for n in 0 1 2; do
  python3 -c "import json; print(json.dumps({'events':[{'t':0.05+0.02*i,'type':'key','name':'t'} for i in range($n)]}))" > "$TMP/place$n.json"
  place=$(echo "coast lake mesa" | cut -d' ' -f$((n + 1)))
  for theme in light dark; do
    for sc in $SCENES; do
      set -- $(shot "$sc")
      scene=$([ "$1" = "--" ] || echo "--scene $1")
      $APP --script "$TMP/place$n.json" $scene --theme $theme --snap "$OUT/$place-$theme-$sc.png" --at "$2" >/dev/null 2>&1
    done
    $APP --script "$TMP/place$n.json" --scene today --theme $theme --snap "$OUT/$place-$theme-today.png" --at 0.6 >/dev/null 2>&1
    ffmpeg -loglevel error -y $(for sc in $SCENES today; do printf -- "-i $OUT/$place-$theme-$sc.png "; done) \
      -filter_complex "hstack=11,scale=iw/2:-1" "$OUT/sheet-$place-$theme.png"
  done
done
# Each state with the three places side by side, light over dark, to compare them directly.
for sc in $SCENES today; do
  ffmpeg -loglevel error -y $(for th in light dark; do for p in coast lake mesa; do printf -- "-i $OUT/$p-$th-$sc.png "; done; done) \
    -filter_complex "[0][1][2]hstack=3[a];[3][4][5]hstack=3[b];[a][b]vstack=2,scale=iw/2:-1" "$OUT/places-$sc.png"
done
$APP --scene controls --snap "$OUT/coast-light-controls.png" --at 0.6 >/dev/null 2>&1

$APP --scene last-minute --script scripts/bell.json --video "$OUT/focus-to-bell.mp4" --fps 30 >/dev/null 2>&1
$APP --scene night-minute --script scripts/night.json --video "$OUT/long-break.mp4" --fps 30 >/dev/null 2>&1

for sc in $(echo $SCENES | sed "s/bell-bloom //") today; do
  set -- $(shot "$sc")
  scene=$([ "$1" = "--" ] || echo "--scene $1")
  cpu=$($APP $scene --snap "$TMP/t.png" --at "$2" 2>&1 | grep 'frame cpu' | sed 's/.*frame cpu/frame cpu/')
  rm -rf "$TMP/f" && mkdir "$TMP/f"
  $APP $scene --out "$TMP/f" --start 0.3 --frames 120 --scale 1 >/dev/null 2>&1
  redraws=$(( $(cd "$TMP/f" && md5 -q frame_*.png | uniq | wc -l) - 1 ))
  printf '%-12s %s; %s frames differ from the one before, of 120 over 2 s\n' "$sc" "$cpu" "$redraws" >> "$OUT/frames.txt"
done
rm -rf "$TMP"
cat "$OUT/frames.txt"
ls "$OUT" | wc -l
