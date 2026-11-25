First release of **MACN-cheese**, a native macOS (Mac Catalyst, Apple Silicon, macOS 26+) port of [iPadn](https://github.com/mengangs/ipadn) by mengangs (GPL-2.0).

### Install
1. Download `MACN-cheese-0.1.0-macOS-arm64.zip`, unzip it, and move `MACN-cheese.app` to Applications.
2. The app is **ad-hoc signed and not notarized**. **Right-click › Open › Open** the first time. You can also use System Settings › Privacy & Security › Open Anyway, or run `xattr -dr com.apple.quarantine /Applications/MACN-cheese.app`.
3. Sign in with your Apple ID inside the app. It's stored in your login keychain.

### Works
Apple ID sign-in, search, version history and old versions, purchased list, multiple regions, download queue, package tools. Finished `.ipa`/`.pkg` files are saved to `~/Downloads/MACN-cheese` (configurable) with **Show in Finder**.

### Not on Mac
Installing apps onto a device, Live Activities and widgets, on-device pairing, TestFlight (needs a closed-source upstream module), and processing protected Mac App Store `.pkg` files.
