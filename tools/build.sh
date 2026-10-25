#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Build the Rust core and the APK, run host tests, stage APK + checksum in dist/.
# Same prerequisites as tools/build.ps1. Usage: tools/build.sh [--release]
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
task=assembleDebug
[ "${1:-}" = "--release" ] && task=assembleRelease

(cd "$root/core" && cargo test --locked)
(cd "$root/android" && ./gradlew "$task" --console=plain)

version=$(sed -n 's/.*versionName = "\(.*\)".*/\1/p' "$root/android/app/build.gradle.kts")
mkdir -p "$root/dist"
if [ "$task" = assembleRelease ]; then
  src=$(ls "$root"/android/app/build/outputs/apk/release/*.apk | head -n1)
  case "$src" in *unsigned*) name="usbip-bridge-$version-release-unsigned.apk" ;; *) name="usbip-bridge-$version.apk" ;; esac
else
  src="$root/android/app/build/outputs/apk/debug/app-debug.apk"
  name="usbip-bridge-$version-debug.apk"
fi
cp "$src" "$root/dist/$name"
(cd "$root/dist" && sha256sum "$name" > SHA256SUMS && cat SHA256SUMS)
