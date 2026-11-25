# MACN-cheese 🧀

**MACN-cheese is a native macOS port of [iPadn](https://github.com/mengangs/ipadn)** by mengangs: search the App Store, browse version history, and download `.ipa` / `.pkg` files for apps your Apple ID owns, including older versions and other regions.

It's built from iPadn's SwiftUI source as a Mac Catalyst app (Apple Silicon, macOS 26 or later). This is the same app, adapted for the Mac. It isn't a wrapper and doesn't use an iOS emulator.

> MACN-cheese is an unofficial port and isn't affiliated with iPadn or Apple. Credit for the app goes to mengangs and the iPadn contributors. Licensed GPL-2.0, same as upstream (see [LICENSE](LICENSE)). The original READMEs are kept in [`docs/UPSTREAM_README_EN.md`](docs/UPSTREAM_README_EN.md) and [`docs/UPSTREAM_README.md`](docs/UPSTREAM_README.md).

## Download

Get `MACN-cheese-<version>-macOS-arm64.zip` from [Releases](https://github.com/OBLIVIONck/MACN-cheese/releases), unzip it, and move `MACN-cheese.app` to `/Applications` or `~/Applications`.

The app is **ad-hoc signed, not notarized**. On first launch, **right-click the app, choose Open, then choose Open again**. If macOS still refuses, go to System Settings › Privacy & Security and click "Open Anyway", or run:

```bash
xattr -dr com.apple.quarantine /Applications/MACN-cheese.app
```

## What works on the Mac

- Apple ID sign-in (stored in your login keychain), multiple accounts
- Search, app details, prices, reviews, multi-region stores
- Version history and downloading old versions
- Purchased list
- Download queue: pause, resume, retry
- Package library, package info and compare, FairPlay "Pro mode" tools where the Mac supports them
- Wi-Fi transfer page, account backup and restore, watch list and update checks

**Downloads on the Mac:** nothing gets installed. Finished `.ipa` / `.pkg` files are copied to a save folder (default `~/Downloads/MACN-cheese`, change it in Settings › Downloads › Mac). Click **Show in Finder** on any finished download.

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

In-app update checks look at **this** repo's releases, never upstream iPadn's.

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

## Credits

- **[iPadn](https://github.com/mengangs/ipadn)** by mengangs and contributors (GPL-2.0). This port is based on it.
- Third-party components are listed in [`iPadn-iOS/THIRD_PARTY_NOTICES.md`](iPadn-iOS/THIRD_PARTY_NOTICES.md).
