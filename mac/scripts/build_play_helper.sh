#!/bin/bash
# Build macn-play (Google Play helper, Rust) for Apple Silicon into mac/.build/macn-play.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --manifest-path PlayHelper/Cargo.toml --target-dir .build/play-target
cp .build/play-target/release/macn-play .build/macn-play
echo "built $(pwd)/.build/macn-play"
