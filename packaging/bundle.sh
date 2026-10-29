#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
app="$root/dist/Polymath.app"
binary="$root/../reader/target/release/polymath"
[ -f "$binary" ] || { echo 'Build Polymath with cargo build --locked --release first.' >&2; exit 1; }
# An ad-hoc signature identifies each build by its hash. macOS then treats an
# update as a different app for microphone, Input Monitoring and Accessibility.
# Prefer a persistent local signing identity when one has been installed.
signing_identity=${POLYMATH_CODESIGN_IDENTITY:-Polymath Local Code Signing}
if [ "$signing_identity" = 'Polymath Local Code Signing' ] &&
    ! /usr/bin/security find-certificate -c "$signing_identity" -p 2>/dev/null | /usr/bin/grep -q 'BEGIN CERTIFICATE'; then
    signing_identity=-
fi
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/licenses"
cp "$binary" "$app/Contents/MacOS/Polymath"
/usr/bin/swiftc -O "$root/packaging/PolymathRecorder.swift" -o "$app/Contents/MacOS/PolymathRecorder"
/usr/bin/swiftc -O "$root/packaging/PolymathGlobalDictation.swift" "$root/packaging/PolymathClipboard.swift" -o "$app/Contents/MacOS/PolymathGlobalDictation"
# These helpers use the app's designated requirement so that macOS recognizes
# the same privacy grant when Polymath launches them as child processes.
/usr/bin/codesign --force --sign "$signing_identity" --identifier local.polymath.app "$app/Contents/MacOS/PolymathGlobalDictation"
/usr/bin/codesign --force --sign "$signing_identity" --identifier local.polymath.app "$app/Contents/MacOS/PolymathRecorder"
speech="$root/../speech-engine"
model="$speech/models/ggml-small-q5_1.bin"
cli="$speech/build/bin/whisper-cli"
[ -f "$model" ] && [ -f "$cli" ] || { echo 'Build speech-engine and download its small-q5_1 model before bundling.' >&2; exit 1; }
mkdir -p "$app/Contents/Resources/models" "$app/Contents/Resources/licenses/whisper"
cp "$cli" "$app/Contents/MacOS/PolymathWhisper"
/usr/bin/codesign --force --sign "$signing_identity" --identifier local.polymath.app "$app/Contents/MacOS/PolymathWhisper"
cp "$model" "$app/Contents/Resources/models/ggml-small-q5_1.bin"
cp "$speech/LICENSE" "$app/Contents/Resources/licenses/whisper/LICENSE"
cp "$root/assets/licenses/Whisper-MIT.txt" "$app/Contents/Resources/licenses/whisper/OPENAI-LICENSE"
if [ -d "$root/../reader/assets/models" ]; then
    /usr/bin/ditto "$root/../reader/assets/models" "$app/Contents/Resources/models"
fi
cp "$root/assets/icons/polymath.icns" "$app/Contents/Resources/Polymath.icns"
mkdir -p "$app/Contents/Resources/licenses/polymath/fonts"
cp "$root/../reader/assets/fonts/OFL-Inter.txt" "$app/Contents/Resources/licenses/polymath/fonts/"
mkdir -p "$app/Contents/Resources/licenses/egui-shadcn"
cp "$root/vendor/egui-shadcn/LICENSE-MIT" "$app/Contents/Resources/licenses/egui-shadcn/LICENSE-MIT"
mkdir -p "$app/Contents/Resources/licenses/craft-quit"
cp "$root/assets/licenses/craft-quit/LICENSE-MIT" "$root/assets/licenses/craft-quit/LICENSE-APACHE" "$app/Contents/Resources/licenses/craft-quit/"
python3 "$root/packaging/plist.py" "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"
for source in reader writer sheets slides lightcraft; do
    target="$app/Contents/Resources/licenses/$source"
    mkdir -p "$target"
    for name in LICENSE LICENSE-MIT LICENSE-APACHE NOTICE ATTRIBUTION.md ATTRIBUTION.toml; do
        [ ! -f "$root/../$source/$name" ] || cp "$root/../$source/$name" "$target/"
    done
    for name in assets/licenses licenses docs/licenses; do
        [ ! -d "$root/../$source/$name" ] || /usr/bin/ditto "$root/../$source/$name" "$target/$name"
    done
done
python3 "$root/packaging/copy_licenses.py" "$app/Contents/Resources/licenses"
cp "$root/NOTICE" "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$root/ATTRIBUTION.md" "$app/Contents/Resources/licenses/"
/usr/bin/codesign --force --sign "$signing_identity" --identifier local.polymath.app "$app"
/usr/bin/codesign --verify --deep --strict "$app"
printf '%s\n' "$app"
