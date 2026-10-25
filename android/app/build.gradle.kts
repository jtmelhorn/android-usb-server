import java.io.File
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val ndkVersionPinned = "27.2.12479018"

android {
    namespace = "io.github.jtmelhorn.usbipbridge"
    compileSdk = 34
    ndkVersion = ndkVersionPinned

    defaultConfig {
        applicationId = "io.github.jtmelhorn.usbipbridge"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a") }
    }

    // Release signing only when a keystore is supplied; no keys live in the repository.
    val ks = System.getenv("USBIP_KEYSTORE")
    signingConfigs {
        if (ks != null) {
            create("release") {
                storeFile = file(ks)
                storePassword = System.getenv("USBIP_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("USBIP_KEY_ALIAS")
                keyPassword = System.getenv("USBIP_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (ks != null) signingConfig = signingConfigs.getByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    sourceSets["main"].jniLibs.srcDir(layout.buildDirectory.dir("rustJniLibs"))
}

// ---- Rust core cross-build -------------------------------------------------------------------

val rustTargets = mapOf(
    "arm64-v8a" to Pair("aarch64-linux-android", "aarch64-linux-android26-clang"),
    "armeabi-v7a" to Pair("armv7-linux-androideabi", "armv7a-linux-androideabi26-clang"),
)
val coreDir = rootProject.projectDir.resolve("../core").canonicalFile

fun cargoExecutable(): String {
    val home = System.getProperty("user.home")
    val exe = if (System.getProperty("os.name").lowercase().contains("win")) "cargo.exe" else "cargo"
    val candidate = File(home, ".cargo/bin/$exe")
    return if (candidate.exists()) candidate.absolutePath else "cargo"
}

fun ndkDir(): File {
    val props = Properties()
    val lp = rootProject.file("local.properties")
    if (lp.exists()) lp.inputStream().use { props.load(it) }
    val sdk = props.getProperty("sdk.dir") ?: System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT")
        ?: error("Android SDK not found: set sdk.dir in local.properties or ANDROID_HOME")
    return File(sdk, "ndk/$ndkVersionPinned")
}

fun hostTag(): String {
    val os = System.getProperty("os.name").lowercase()
    return when {
        os.contains("win") -> "windows-x86_64"
        os.contains("mac") -> "darwin-x86_64"
        else -> "linux-x86_64"
    }
}

val cargoBuild by tasks.registering {
    description = "Cross-compiles the Rust USB/IP core for every packaged ABI."
    inputs.dir(coreDir.resolve("src"))
    inputs.file(coreDir.resolve("Cargo.toml"))
    val outRoot = layout.buildDirectory.dir("rustJniLibs")
    outputs.dir(outRoot)
    doLast {
        val bin = ndkDir().resolve("toolchains/llvm/prebuilt/${hostTag()}/bin")
        val win = hostTag().startsWith("windows")
        for ((abi, t) in rustTargets) {
            val (triple, clang) = t
            val linker = bin.resolve(if (win) "$clang.cmd" else clang)
            check(linker.exists()) { "NDK linker not found: $linker (install NDK $ndkVersionPinned)" }
            val envKey = "CARGO_TARGET_${triple.uppercase().replace('-', '_')}_LINKER"
            exec {
                workingDir = coreDir
                environment(envKey, linker.absolutePath)
                commandLine(cargoExecutable(), "build", "--release", "--locked", "--target", triple)
            }
            val dest = outRoot.get().asFile.resolve(abi)
            dest.mkdirs()
            coreDir.resolve("target/$triple/release/libusbipcore.so").copyTo(dest.resolve("libusbipcore.so"), overwrite = true)
        }
    }
}
tasks.named("preBuild") { dependsOn(cargoBuild) }

dependencies {}
