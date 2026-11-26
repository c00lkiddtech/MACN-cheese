#!/usr/bin/env bash
# Download official ipatool releases and copy (or expect) macn-play into src-tauri/binaries/.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/src-tauri/binaries"
mkdir -p "$BIN"
IPATOOL_VER="${IPATOOL_VER:-2.8.1}"

fetch_ipatool() {
  local goos="$1" goarch="$2" dest="$3"
  local base="https://github.com/majd/ipatool/releases/download/v${IPATOOL_VER}"
  local name="ipatool-${IPATOOL_VER}-${goos}-${goarch}.tar.gz"
  local tmp
  tmp="$(mktemp -d)"
  echo "downloading $name"
  curl -fsSL -o "$tmp/ipatool.tgz" "$base/$name"
  curl -fsSL -o "$tmp/ipatool.tgz.sha256sum" "$base/$name.sha256sum"
  # GitHub may publish a bare hash or "hash  filename".
  hash="$(tr -d '\r' < "$tmp/ipatool.tgz.sha256sum" | awk '{print $1}')"
  echo "$hash  $tmp/ipatool.tgz" | sha256sum -c -
  tar -xzf "$tmp/ipatool.tgz" -C "$tmp"
  local exe
  exe="$(find "$tmp" -type f \( -name 'ipatool' -o -name 'ipatool.exe' \) | head -n1)"
  if [[ -z "$exe" ]]; then
    exe="$(find "$tmp" -type f -name 'ipatool*' ! -name '*.sha256sum' ! -name '*.tgz' ! -name '*.tar.gz' | head -n1)"
  fi
  mkdir -p "$(dirname "$dest")"
  if command -v strip >/dev/null 2>&1 && [[ "$dest" != *.exe ]]; then
    strip -s "$exe" -o "$dest" || cp "$exe" "$dest"
  else
    cp "$exe" "$dest"
  fi
  chmod +x "$dest" || true
  rm -rf "$tmp"
  echo "wrote $dest"
}

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"
if [[ "${PREPARE_ALL:-}" == "1" ]]; then
  fetch_ipatool linux amd64 "$BIN/ipatool-x86_64-unknown-linux-gnu"
  fetch_ipatool windows amd64 "$BIN/ipatool-x86_64-pc-windows-msvc.exe"
elif [[ "${PREPARE_WINDOWS:-}" == "1" ]]; then
  fetch_ipatool windows amd64 "$BIN/ipatool-x86_64-pc-windows-msvc.exe"
else
  if [[ "$HOST_OS" == "Linux" && ( "$HOST_ARCH" == "x86_64" || "$HOST_ARCH" == "amd64" ) ]]; then
    fetch_ipatool linux amd64 "$BIN/ipatool-x86_64-unknown-linux-gnu"
  elif [[ "$HOST_OS" == MINGW* || "$HOST_OS" == MSYS* || "$HOST_OS" == CYGWIN* || "$HOST_OS" == Windows_NT ]]; then
    fetch_ipatool windows amd64 "$BIN/ipatool-x86_64-pc-windows-msvc.exe"
  fi
fi

copy_play() {
  local src="$1" dest="$2"
  if [[ -f "$src" ]]; then
    cp "$src" "$dest"
    chmod +x "$dest" || true
    echo "wrote $dest from $src"
  fi
}

copy_play "$ROOT/.build/play-target/x86_64-unknown-linux-gnu/release/macn-play" "$BIN/macn-play-x86_64-unknown-linux-gnu"
copy_play "$ROOT/.build/play-target/release/macn-play" "$BIN/macn-play-x86_64-unknown-linux-gnu"
copy_play "$ROOT/.build/play-target/x86_64-pc-windows-msvc/release/macn-play.exe" "$BIN/macn-play-x86_64-pc-windows-msvc.exe"
copy_play "${PLAY_HELPER_LINUX:-}" "$BIN/macn-play-x86_64-unknown-linux-gnu"
copy_play "${PLAY_HELPER_WINDOWS:-}" "$BIN/macn-play-x86_64-pc-windows-msvc.exe"


# Bundle merge/sign jars next to the Tauri resources path.
RES="$ROOT/src-tauri/resources/AndroidTools"
mkdir -p "$RES"
cp -f "$ROOT/helpers/Vendor/apkeditor/APKEditor.jar" "$RES/APKEditor.jar"
cp -f "$ROOT/helpers/Vendor/apkeditor/LICENSE" "$RES/APKEditor-LICENSE"
cp -f "$ROOT/helpers/Vendor/uber-apk-signer/uber-apk-signer.jar" "$RES/uber-apk-signer.jar"
cp -f "$ROOT/helpers/Vendor/uber-apk-signer/LICENSE" "$RES/uber-apk-signer-LICENSE"

echo "sidecars in $BIN:"
ls -lh "$BIN" || true
