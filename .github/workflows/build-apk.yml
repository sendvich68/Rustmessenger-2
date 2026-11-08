name: Build APK

on:
  push:
    branches: [ main ]
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-linux-android

      - name: Setup Android NDK
        uses: nttld/setup-ndk@v1
        with:
          ndk-version: r25c

      - name: Setup Java
        uses: actions/setup-java@v4
        with:
          distribution: temurin
          java-version: 17

      - name: Install cargo-ndk
        run: cargo install cargo-ndk

      - name: Build Rust binary
        run: |
          cargo ndk -t arm64-v8a build --release
          mkdir -p android/app/src/main/assets
          cp target/aarch64-linux-android/release/rust_messenger \
             android/app/src/main/assets/

      - name: Setup Android SDK
        uses: android-actions/setup-android@v3

      - name: Build APK
        run: |
          cd android
          chmod +x gradlew
          ./gradlew assembleRelease

      - name: Upload APK
        uses: actions/upload-artifact@v4
        with:
          name: RustMessenger-APK
          path: android/app/build/outputs/apk/release/app-release.apk
