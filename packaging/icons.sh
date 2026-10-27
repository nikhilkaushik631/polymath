#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
icons="$root/assets/icons"
work="$root/target/icons"
mkdir -p "$work/Polymath.iconset"
# SVG is the editable master; each macOS representation is rendered at its own size.
for size in 16 32 128 256 512; do
    for factor in 1 2; do
        pixels=$((size * factor))
        inner=$((pixels * 824 / 1024))
        suffix=""
        if [ "$factor" = 2 ]; then suffix="@2x"; fi
        rsvg-convert --width "$inner" --height "$inner" --page-width "$pixels" --page-height "$pixels" --left "$(((pixels-inner)/2))" --top "$(((pixels-inner)/2))" "$icons/polymath.svg" -o "$work/Polymath.iconset/icon_${size}x${size}${suffix}.png"
    done
done
cp "$work/Polymath.iconset/icon_512x512@2x.png" "$icons/polymath-1024.png"
iconutil -c icns "$work/Polymath.iconset" -o "$icons/polymath.icns"
