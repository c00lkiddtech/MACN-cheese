#!/bin/bash
# Assemble, sign (ad-hoc) and optionally install MACN-cheese.app.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD; SRC=$ROOT/iPadn-iOS; OUT=$ROOT/mac/.build
APP=$OUT/MACN-cheese.app
NAME=MACN-cheese
BUNDLE_ID=com.oblivion.macn-cheese
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$OUT/$NAME" "$APP/Contents/MacOS/$NAME"
# Google Play helper (Rust, EFForg/rs-google-play + apkeep flow) for the Android tab.
[ -x "$OUT/macn-play" ] || "$ROOT/mac/scripts/build_play_helper.sh"
cp "$OUT/macn-play" "$APP/Contents/MacOS/macn-play"
R=$APP/Contents/Resources
mkdir -p "$R/Licenses"
cp "$ROOT/mac/Vendor/apkeep/LICENSE" "$R/Licenses/apkeep-MIT.txt"
cp "$ROOT/mac/Vendor/rs-google-play/gpapi/LICENSE" "$R/Licenses/rs-google-play-MIT.txt"
cp "$ROOT/LICENSE" "$R/Licenses/MACN-cheese-iPadn-GPL-2.0.txt"
python3 "$SRC/scripts/assemble_runtime.py" >/dev/null
cp -R "$SRC/iPadn/Resources/SAPRuntime" "$SRC/iPadn/Resources/RevealAnimations" "$R/"
cp "$SRC/iPadn/Resources/app-name-directory.json" "$SRC/iPadn/Resources/PrivacyInfo.xcprivacy" "$SRC/iPadn/Resources/ZIPFoundation-MIT.txt" "$R/"
cp "$SRC"/iPadn/Resources/AlternateIcons/*.png "$R/" 2>/dev/null || true
for l in "$SRC"/iPadn/Resources/*.lproj; do
  mkdir -p "$R/$(basename "$l")"
  for f in "$l"/*.strings; do sed "s/iPadn/$NAME/g" "$f" > "$R/$(basename "$l")/$(basename "$f")"; done
done
MACN_BRAND="$NAME" python3 "$ROOT/mac/scripts/xcstrings2strings.py" "$R" "$SRC/iPadn/Resources/Localizable.xcstrings" "$SRC/iPadn/Resources/InfoPlist.xcstrings"
# No actool without Xcode: ship asset-catalog images as loose files.
for set in BrandMark FlatBrandMark; do
  cp "$SRC/iPadn/Resources/Assets.xcassets/$set.imageset/brand.png" "$R/$set.png"
done
# App icon (.icns via iconutil).
IS=$OUT/AppIcon.iconset; rm -rf "$IS"; mkdir -p "$IS"
ICON=$SRC/iPadn/Resources/Assets.xcassets/AppIcon.appiconset/icon-1024.png
[ -f "$ROOT/mac/Resources/AppIcon-1024.png" ] && ICON=$ROOT/mac/Resources/AppIcon-1024.png
for s in 16 32 128 256 512; do
  sips -z $s $s "$ICON" --out "$IS/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) "$ICON" --out "$IS/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$IS" -o "$R/AppIcon.icns"
python3 "$ROOT/mac/scripts/make_plist.py" "$SRC/iPadn/Info.plist" "$APP/Contents/Info.plist" "$NAME" "$BUNDLE_ID"
printf 'APPL????' > "$APP/Contents/PkgInfo"
codesign --force --deep --sign - --entitlements "$ROOT/mac/MACN-cheese.entitlements" "$APP"
echo "built $APP"
if [ "${INSTALL:-0}" = 1 ]; then
  mkdir -p "$HOME/Applications"; rm -rf "$HOME/Applications/$NAME.app"
  ditto "$APP" "$HOME/Applications/$NAME.app"; echo "installed ~/Applications/$NAME.app"
fi
