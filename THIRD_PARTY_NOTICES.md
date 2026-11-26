# Third-party notices

WINN (Windows) and LINCE (Linux) are licensed GPL-2.0-or-later (see `LICENSE`). They ship two helper programs and several bundled libraries/tools.

## iPadn

[iPadn](https://github.com/mengangs/ipadn) by mengangs and contributors, GPL-2.0-or-later. WINN/LINCE is an unofficial Windows/Linux companion to the MACN-cheese macOS port of iPadn. It is not affiliated with iPadn or Apple.

## ipatool

[ipatool](https://github.com/majd/ipatool) by Majd Alfhaily, MIT License. Official release binaries (currently v2.8.1) are downloaded at build time by `scripts/prepare-sidecars.sh` and bundled as a sidecar. The license text is at https://github.com/majd/ipatool/blob/main/LICENSE.md (MIT).

## macn-play

`helpers/PlayHelper` is a separate program, GPL-3.0-or-later (it embeds Aurora OSS / Calyx device profiles via `gpapi`). Source lives in this repository. It is executed as a child process, not statically linked into the UI.

### apkeep (MIT)

The Google Play download flow follows [apkeep](https://github.com/EFForg/apkeep) by William Budington / Electronic Frontier Foundation, MIT. License: `helpers/Vendor/apkeep/LICENSE`.

### rs-google-play (MIT)

[rs-google-play](https://github.com/EFForg/rs-google-play) (`gpapi`, `googleplay-protobuf`) by Bill Budington / EFF and David Weinstein, MIT, vendored in `helpers/Vendor/rs-google-play`. License: `helpers/Vendor/rs-google-play/LICENSE`.

### Aurora Store profiles (GPL-3.0-or-later)

Device profiles and the anonymous session dispenser come from [Aurora Store](https://gitlab.com/AuroraOSS/AuroraStore) by AuroraOSS, GPL-3.0-or-later. No personal Google account is used.

### APKEditor (Apache-2.0)

[APKEditor](https://github.com/REAndroid/APKEditor) by REAndroid, Apache-2.0, bundled as `src-tauri/resources/AndroidTools/APKEditor.jar` (source copy of the license: `helpers/Vendor/apkeditor/LICENSE`). Used only when Java is available, to merge split APKs.

### uber-apk-signer (Apache-2.0)

[uber-apk-signer](https://github.com/patrickfav/uber-apk-signer) by Patrick Favre-Bulle, Apache-2.0, bundled as `src-tauri/resources/AndroidTools/uber-apk-signer.jar` (license: `helpers/Vendor/uber-apk-signer/LICENSE`). Signs the merged APK with a local debug key.

## Tauri, Rust and npm crates

The desktop shell uses [Tauri](https://tauri.app/) 2 and the Rust/npm crates declared in `src-tauri/Cargo.toml` / `package.json` (typically MIT or Apache-2.0). Their license texts are available from those crates’ upstream repositories.

This repository does not include Apple or Google account credentials, 2FA codes, signing certificates, or private keys.
