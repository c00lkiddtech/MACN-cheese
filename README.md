# MACN-cheese 🧀

**MACN-cheese is a native macOS port of [iPadn](https://github.com/mengangs/ipadn)** by mengangs: search the App Store, browse version history, and download `.ipa` / `.pkg` files for apps your Apple ID owns, including older versions and other regions.

It's built from iPadn's SwiftUI source as a Mac Catalyst app (Apple Silicon, macOS 26 or later). This is the same app, adapted for the Mac. It isn't a wrapper and doesn't use an iOS emulator.

This repository is also the home of **WINN** (Windows) and **LINCE** (Linux), a shared Tauri 2 companion in [`desktop/`](desktop/).

> MACN-cheese is an unofficial port and isn't affiliated with iPadn or Apple. Credit for the app goes to mengangs and the iPadn contributors. Licensed GPL-2.0, same as upstream (see [LICENSE](LICENSE)). The original READMEs are kept in [`docs/UPSTREAM_README_EN.md`](docs/UPSTREAM_README_EN.md) and [`docs/UPSTREAM_README.md`](docs/UPSTREAM_README.md).

## Platforms

| Name | Platform | Status | Where to get it |
|---|---|---|---|
| **MACN-cheese** | macOS (Apple Silicon, 26+) | Available | [Releases](https://github.com/c00lkiddtech/MACN-cheese/releases) (`MACN-cheese-*-macOS-arm64.zip`) |
| **WINN** | Windows | Available | [Releases](https://github.com/c00lkiddtech/MACN-cheese/releases) (`WINN_*-x64-setup.exe`) |
| **LINCE** | Linux | Available | [Releases](https://github.com/c00lkiddtech/MACN-cheese/releases) (`.deb` or `.AppImage`) |
| **IPADN+** | iOS / iPadOS | Planned | — |
| **ANDN-** | Android | Planned | — |
| **WEBN** | Web (GitHub Pages) | Planned | — |

All of them only **download** packages (`.ipa`, `.apk`, `.pkg`). Nothing is installed, and FairPlay / other DRM is never stripped.

## Download

### Mac

Get `MACN-cheese-<version>-macOS-arm64.zip` from [Releases](https://github.com/c00lkiddtech/MACN-cheese/releases), unzip it, and move `MACN-cheese.app` to `/Applications` or `~/Applications`.

The app is **ad-hoc signed, not notarized**. On first launch, **right-click the app, choose Open, then choose Open again**. If macOS still refuses, go to System Settings › Privacy & Security and click "Open Anyway", or run:

```bash
xattr -dr com.apple.quarantine /Applications/MACN-cheese.app
```

### Windows (WINN) and Linux (LINCE)

Grab the WINN installer or the LINCE `.deb` / `.AppImage` from the same [Releases](https://github.com/c00lkiddtech/MACN-cheese/releases) page. These builds are unsigned, so Windows SmartScreen may ask you to run anyway, and the AppImage may need `chmod +x`. Merging split Android packages needs a JRE or JDK.

Source and build notes for WINN and LINCE are in [`desktop/README.md`](desktop/README.md).

## What works on the Mac

- Apple ID sign-in (stored in your login keychain), multiple accounts
- Search, app details, prices, reviews, multi-region stores
- Version history and downloading old versions
- Purchased list
- Download queue: pause, resume, retry
- Package library, package info and compare, FairPlay "Pro mode" tools where the Mac supports them
- Wi-Fi transfer page, account backup and restore, watch list and update checks

**Downloads on the Mac:** nothing gets installed. Finished `.ipa` / `.pkg` files are copied to a save folder (default `~/Desktop`, change it in Settings › Downloads › Mac). Click **Show in Finder** on any finished download.

## Disabled on the Mac

| Feature | Why |
|---|---|
| Installing apps (one-tap / local / native install, auto-install, installed-apps list, installation settings) | These are iPhone/iPad-only (itms-services, installd, device pairing). The Mac saves files instead. |
| Live Activities, Lock Screen and Dynamic Island download progress, widgets | ActivityKit/WidgetKit don't exist in Mac Catalyst. The extension isn't built. |
| On-device pairing (iOS 27) | iPhone-only |
| TestFlight tab | Depends on a closed-source `AppleAuth` module that isn't in the public repo |
| Processing protected Mac App Store `.pkg` files (MacPKGBridge) | Optional Go/Unicorn component, not built yet for this release. Plain downloads still save. |
| Telegram / donation links | Replaced with this repo's links. Please support upstream iPadn directly. |
| Translation popover | Uses direct in-app translation instead, because the popover API isn't available in Catalyst |

In-app update checks look at **this** repo's releases, never upstream iPadn's. WINN and LINCE open this same releases page from their Updates button.

## Building from source (no Xcode needed)

Requirements: Apple Silicon Mac, Command Line Tools (`xcode-select --install`), Python 3, git.

```bash
cd mac
CONFIG=release ./scripts/build.sh        # compiles ZIPFoundation, FairPlayCore and the app
INSTALL=1 ./scripts/bundle.sh            # assembles, ad-hoc signs, installs ~/Applications/MACN-cheese.app
```

Notes:
- Builds use `MacOSX26.5.sdk` from the Command Line Tools. In the 27 SDK, `@State` is a macro whose compiler plugin ships only with Xcode.
- `mac/scripts/xcstrings2strings.py` compiles the String Catalogs (all 11 languages) into `.strings`/`.stringsdict` without Xcode, and rebrands visible text.
- Mac-specific code is in `iPadn-iOS/iPadn/Mac/`: an ActivityKit no-op shim, a login-keychain shim (ad-hoc builds can't use the data-protection keychain), save-folder handling and settings. Shared files are changed only behind `MacPort.isMac` or `#if targetEnvironment(macCatalyst)`, so the iOS build is unaffected.

## Android (Google Play) downloads

The **Android** tab downloads free Google Play apps: paste a package name (`com.example.app`) or a Play Store link, or search, then press **Download**. By default you get **one installable `.apk`**: base and split APKs are merged with APKEditor, then zipaligned and signed with a local debug key (created once in `~/Library/Application Support/MACN-cheese/android-debug.jks`) by uber-apk-signer. Merging needs Java (`brew install openjdk`); on Macs with SIP/AMFI off, a small bundled shim (`mac/Tools/jitfix`) works around the JDK-8326663 HotSpot crash. `.apks`, `.xapk` and a folder of split APKs are still available, and **Convert .apks / .xapk to .apk…** merges files you already have (or run `macn-play merge file.apks`). Because the merged APK is re-signed, it can't update a copy installed from Google Play; uninstall that first. It uses the bundled `macn-play` helper (`mac/PlayHelper`, Rust, built by `mac/scripts/build_play_helper.sh`) with anonymous sessions only.

Nothing gets installed on the Mac: finished `.ipa`, `.pkg` and `.apk` files are saved to `~/Desktop` by default (Settings › Downloads › Mac to change it). WINN and LINCE use the same helper, as a separate process, and save to Downloads or Desktop.

## Credits

- **[iPadn](https://github.com/mengangs/ipadn)** by mengangs and contributors (GPL-2.0). This port is based on it.
- **[ipatool](https://github.com/majd/ipatool)** by Majd Alfhaily (MIT). WINN and LINCE use a bundled copy for App Store sign-in, search and IPA downloads.
- **[apkeep](https://github.com/EFForg/apkeep)** by William Budington / Electronic Frontier Foundation (MIT, license in [`mac/Vendor/apkeep/LICENSE`](mac/Vendor/apkeep/LICENSE)). The Android tab's Google Play download flow (anonymous login, base + split APKs) follows apkeep.
- **[rs-google-play](https://github.com/EFForg/rs-google-play)** (`gpapi`, `googleplay-protobuf`) by Bill Budington / EFF and David Weinstein (MIT, vendored in [`mac/Vendor/rs-google-play`](mac/Vendor/rs-google-play)). The Google Play protocol used by the bundled `macn-play` helper.
- **[APKEditor](https://github.com/REAndroid/APKEditor)** by REAndroid (Apache-2.0, jar and license in [`mac/Vendor/apkeditor`](mac/Vendor/apkeditor)). Merges base + split APKs into one `.apk`.
- **[uber-apk-signer](https://github.com/patrickfav/uber-apk-signer)** by Patrick Favre-Bulle (Apache-2.0, jar and license in [`mac/Vendor/uber-apk-signer`](mac/Vendor/uber-apk-signer)). Zipaligns and signs the merged `.apk` with a local debug key.
- **[Aurora Store](https://gitlab.com/AuroraOSS/AuroraStore)** by AuroraOSS: device profiles and the anonymous session dispenser (GPL-3.0-or-later). No personal Google account is used.
- Third-party components are listed in [`iPadn-iOS/THIRD_PARTY_NOTICES.md`](iPadn-iOS/THIRD_PARTY_NOTICES.md) and [`desktop/THIRD_PARTY_NOTICES.md`](desktop/THIRD_PARTY_NOTICES.md).
