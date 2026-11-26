# WINN & LINCE

**WINN** (Windows) and **LINCE** (Linux) live in the [MACN-cheese](https://github.com/c00lkiddtech/MACN-cheese) monorepo, in this `desktop/` folder. They are a shared [Tauri 2](https://tauri.app/) app for downloading App Store `.ipa` / `.pkg` files and Google Play `.apk` files, and are companions to the macOS MACN-cheese app (a port of [iPadn](https://github.com/mengangs/ipadn)).

This is an unofficial companion. It isn't affiliated with iPadn, Apple, or Google.

## What it does

- **App Store tab** — sign in with an Apple ID (including 2FA), search, and download IPAs you already own. Sign-in, search and download are performed by a bundled copy of [ipatool](https://github.com/majd/ipatool) (MIT), running as a separate program. After login, the session is stored in the OS keyring.
- **Android tab** — search and download free Google Play apps with the bundled `macn-play` helper, using anonymous [Aurora OSS](https://gitlab.com/AuroraOSS/AuroraStore) sessions. Your Google account is never used. If Java is installed, split packages are merged into one `.apk` with APKEditor and signed with a local debug key. Without Java, the app saves `.apks` and tells you why.
- **Downloads only** — finished files go to your **Downloads** or **Desktop** folder. Nothing is ever installed.
- **No DRM stripping** — FairPlay-protected IPAs stay protected.
- **Updates** — the in-app Updates button opens [this repo's releases](https://github.com/c00lkiddtech/MACN-cheese/releases).

## Install

Get the latest **WINN** (Windows NSIS installer) or **LINCE** (`.deb` / `.AppImage`) build from the [MACN-cheese Releases](https://github.com/c00lkiddtech/MACN-cheese/releases) page, not from a separate repository.

Merging split Android packages needs a JRE or JDK (`java` on your `PATH`, or `JAVA_HOME`).

## Build from source

From the repository root:

```bash
cd desktop
npm install
bash scripts/prepare-sidecars.sh
cargo build --release --manifest-path helpers/PlayHelper/Cargo.toml --target-dir .build/play-target
bash scripts/prepare-sidecars.sh
npm run tauri -- build
```

Requirements: Rust, Node 20+, and on Linux the usual Tauri packages (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libssl-dev`, `patchelf`, and friends). GitHub Actions builds LINCE on Ubuntu and WINN on Windows (see [`.github/workflows/desktop.yml`](../.github/workflows/desktop.yml)). Tag a `v*` release to attach both artifacts.

## Licenses

The UI is **GPL-2.0-or-later**, same family as iPadn and MACN-cheese (see the repo [LICENSE](../LICENSE)). `macn-play` is a **separate program**, licensed **GPL-3.0-or-later**, because it embeds Aurora OSS device profiles. Credits and license texts are in the app’s Credits tab and in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md):

- [iPadn](https://github.com/mengangs/ipadn) — GPL-2.0-or-later, mengangs and contributors
- [ipatool](https://github.com/majd/ipatool) — MIT, Majd Alfhaily
- [apkeep](https://github.com/EFForg/apkeep) — MIT, William Budington / EFF
- [rs-google-play](https://github.com/EFForg/rs-google-play) — MIT, Bill Budington / EFF and David Weinstein
- [Aurora Store](https://gitlab.com/AuroraOSS/AuroraStore) — GPL-3.0-or-later, AuroraOSS (profiles and anonymous dispenser)
- [APKEditor](https://github.com/REAndroid/APKEditor) — Apache-2.0, REAndroid
- [uber-apk-signer](https://github.com/patrickfav/uber-apk-signer) — Apache-2.0, Patrick Favre-Bulle

## Limits (MVP)

- iPadn’s version-history picker, SAP/StoreProof path, and TestFlight tab are not included. IPA downloads use ipatool’s public protocol.
- The app never auto-purchases paid App Store or Play apps.
- ipatool’s login flags can briefly expose a password in the process list; the lasting session lives in the OS keyring.
- Large IPAs stay FairPlay-encrypted. That is expected.
