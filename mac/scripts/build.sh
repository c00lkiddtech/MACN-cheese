#!/bin/bash
# Build MACN-cheese (Mac Catalyst port of iPadn) with only the Command Line Tools.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
SRC=$ROOT/iPadn-iOS
OUT=$ROOT/mac/.build
# SDK 27 turns @State into a macro whose plugin ships only with Xcode, so use 26.5.
SDK=${SDK:-/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk}
TARGET=arm64-apple-ios26.0-macabi
IOSSUP=$SDK/System/iOSSupport
CFG=${CONFIG:-release}
OPT=$([ "$CFG" = debug ] && echo -Onone || echo -O)
COMMON=(-target $TARGET -sdk "$SDK" -Fsystem "$IOSSUP/System/Library/Frameworks" -I "$IOSSUP/usr/include" -L "$IOSSUP/usr/lib" -swift-version 5)
mkdir -p "$OUT/mods" "$OUT/obj"
STEP=${1:-all}

zip() {
  [ -d "$ROOT/mac/deps/ZIPFoundation" ] || git clone -q --depth 1 --branch 0.9.19 https://github.com/weichsel/ZIPFoundation.git "$ROOT/mac/deps/ZIPFoundation"
  swiftc "${COMMON[@]}" $OPT -parse-as-library -module-name ZIPFoundation -emit-module -emit-module-path "$OUT/mods/ZIPFoundation.swiftmodule" \
    -emit-library -static -o "$OUT/obj/libZIPFoundation.a" "$ROOT"/mac/deps/ZIPFoundation/Sources/ZIPFoundation/*.swift
}
fairplay() {
  clang -target $TARGET -isysroot "$SDK" -O2 -I "$SRC/FairPlayCore/include" -c "$SRC/FairPlayCore/FairPlayCore.c" -o "$OUT/obj/FairPlayCore.o"
}
tfkit() {
  swiftc "${COMMON[@]}" $OPT -parse-as-library -module-name TestFlightKit -I "$OUT/mods" -emit-module -emit-module-path "$OUT/mods/TestFlightKit.swiftmodule" \
    -emit-library -static -o "$OUT/obj/libTestFlightKit.a" $(ls "$SRC"/TestFlightKit/*.swift | grep -v SigningCapability)
}
app() {
  swiftc "${COMMON[@]}" $OPT -parse-as-library -module-name iPadn -D MACN_CHEESE \
    -I "$OUT/mods" -I "$SRC/FairPlayCore/include" -Xcc -I"$SRC/FairPlayCore/include" \
    -L "$OUT/obj" -lZIPFoundation "$OUT/obj/FairPlayCore.o" -Xlinker -weak_framework -Xlinker Translation \
    -Xlinker -rpath -Xlinker /usr/lib/swift \
    $(find "$SRC/iPadn" -name '*.swift' | sort) "$SRC/TestFlightKit/SigningCapability.swift" \
    -o "$OUT/MACN-cheese" > "$OUT/app-build.log" 2>&1 && status=0 || status=$?
  grep -E "error:" "$OUT/app-build.log" | sort -u | head -${ERRMAX:-80} || true
  return $status
}
case $STEP in
  zip) zip;; fairplay) fairplay;; tfkit) tfkit;; app) app;;
  all) zip; fairplay; app; "$ROOT/mac/scripts/bundle.sh";;
esac
