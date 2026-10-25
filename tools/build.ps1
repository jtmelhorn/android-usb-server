# SPDX-License-Identifier: GPL-3.0-or-later
# Build the Rust core and the APK, run host tests, and stage the APK + checksum in dist/.
#
# Prerequisites (see README "Building from source"):
#   JDK 17, Android SDK (platform 34, build-tools 34.0.0, NDK 27.2.12479018), Rust via rustup.
#   Set ANDROID_HOME (or android/local.properties sdk.dir) and JAVA_HOME.
# Optional release signing: USBIP_KEYSTORE, USBIP_KEYSTORE_PASSWORD, USBIP_KEY_ALIAS, USBIP_KEY_PASSWORD.
param([switch]$Release)
# Native tools write progress to stderr; failures are detected through $LASTEXITCODE instead.
$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot

Push-Location "$root/core"
try {
    cargo test --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
} finally { Pop-Location }

$task = if ($Release) { "assembleRelease" } else { "assembleDebug" }
Push-Location "$root/android"
try {
    ./gradlew.bat $task --console=plain
    if ($LASTEXITCODE -ne 0) { throw "gradle $task failed" }
} finally { Pop-Location }

$version = (Select-String -Path "$root/android/app/build.gradle.kts" -Pattern 'versionName = "(.+)"').Matches[0].Groups[1].Value
New-Item -ItemType Directory -Force "$root/dist" | Out-Null
if ($Release) {
    $src = Get-ChildItem "$root/android/app/build/outputs/apk/release/*.apk" | Select-Object -First 1
    $name = if ($src.Name -like "*unsigned*") { "usbip-bridge-$version-release-unsigned.apk" } else { "usbip-bridge-$version.apk" }
} else {
    $src = Get-Item "$root/android/app/build/outputs/apk/debug/app-debug.apk"
    $name = "usbip-bridge-$version-debug.apk"
}
Copy-Item $src.FullName "$root/dist/$name" -Force -ErrorAction Stop
$hash = (Get-FileHash "$root/dist/$name" -Algorithm SHA256).Hash.ToLower()
"$hash  $name" | Out-File -Encoding ascii "$root/dist/SHA256SUMS"
Write-Host "Built dist/$name"
Write-Host "SHA-256 $hash"
