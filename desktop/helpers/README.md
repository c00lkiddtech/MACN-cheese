# Helpers

- `PlayHelper` — `macn-play`, a **separate** GPL-3.0-or-later program used as a sidecar. Ported from MACN-cheese with Linux/Windows path and Java discovery changes. Do not statically link this into the GPL-2.0 UI.
- `Vendor/rs-google-play` — MIT, from EFForg/rs-google-play.
- `Vendor/apkeep` — MIT license text; the helper follows apkeep’s download flow.
- `Vendor/apkeditor` and `Vendor/uber-apk-signer` — Apache-2.0 jars, also copied into `src-tauri/resources/AndroidTools` at build time.

ipatool is not vendored here. `scripts/prepare-sidecars.sh` fetches the official MIT-licensed release.
