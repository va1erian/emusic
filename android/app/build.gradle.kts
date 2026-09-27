import org.gradle.api.tasks.Exec
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

val repoRoot = rootProject.projectDir.parentFile
val rustJniLibs = layout.buildDirectory.dir("rustJniLibs")
val uniffiKotlin = layout.buildDirectory.dir("generated/uniffi")

val cargoNdkBuild = tasks.register<Exec>("cargoNdkBuild") {
    group = "rust"
    description = "Cross-compiles emusic-mobile for the Android ABIs."
    workingDir = repoRoot
    val out = rustJniLibs.get().asFile
    doFirst { out.mkdirs() }
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a", "-t", "x86_64",
        "-o", out.absolutePath,
        "build", "-p", "emusic-mobile", "--lib", "--release",
    )
}

val uniffiBindings = tasks.register<Exec>("uniffiBindings") {
    group = "rust"
    description = "Generates the Kotlin bindings from the cross-compiled library."
    dependsOn(cargoNdkBuild)
    workingDir = repoRoot
    val out = uniffiKotlin.get().asFile
    val library = rustJniLibs.get().asFile.resolve("arm64-v8a/libemusic_mobile.so")
    doFirst { out.mkdirs() }
    commandLine(
        "cargo", "run", "-p", "emusic-mobile", "--features", "cli", "--bin", "uniffi-bindgen", "--",
        "generate", "--library", library.absolutePath, "--language", "kotlin",
        "--out-dir", out.absolutePath, "--no-format",
    )
}

android {
    namespace = "dev.emusic.mobile"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.emusic.mobile"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
    }

    sourceSets["main"].jniLibs.srcDir(rustJniLibs.get().asFile)
    sourceSets["main"].kotlin.srcDir(uniffiKotlin.get().asFile)
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.session)
    implementation(libs.jna) {
        artifact {
            type = "aar"
        }
    }
}

tasks.named("preBuild") {
    dependsOn(uniffiBindings)
}
