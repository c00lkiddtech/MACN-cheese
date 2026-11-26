#!/bin/bash
# Create c00lkiddtech/MACN-cheese (public), push, and publish release v0.1.0.
# Requires `gh` signed in as c00lkiddtech (gh auth login / gh auth switch -u c00lkiddtech).
set -euo pipefail
cd "$(dirname "$0")/../.."
REPO=c00lkiddtech/MACN-cheese
VERSION=${VERSION:-0.1.0}
ZIP=mac/.build/MACN-cheese-$VERSION-macOS-arm64.zip
[ "$(gh api user --jq .login)" = c00lkiddtech ] || { echo "gh is not signed in as c00lkiddtech" >&2; exit 1; }
gh repo view "$REPO" >/dev/null 2>&1 || gh repo create "$REPO" --public \
  --description "MACN-cheese: native macOS port of iPadn (App Store .ipa + Google Play .apk downloader). GPL-2.0." --source . --remote origin
git remote get-url origin >/dev/null 2>&1 || git remote add origin "https://github.com/$REPO.git"
git push -u origin HEAD:main
rm -f "$ZIP"; ditto -c -k --keepParent "$HOME/Applications/MACN-cheese.app" "$ZIP"
gh release view "v$VERSION" -R "$REPO" >/dev/null 2>&1 || \
  gh release create "v$VERSION" "$ZIP" -R "$REPO" --title "MACN-cheese $VERSION" --notes-file mac/RELEASE_NOTES.md
echo "https://github.com/$REPO"
echo "https://github.com/$REPO/releases/tag/v$VERSION"
