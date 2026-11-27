# Stream Tube for Android

Stream Tube for Android brings the Stream Tube Desktop browsing interface to phones. The interface is built in Slint and the Android runtime uses RustFerry. The desktop project was created by Avaneesh Inamdar.

Home opens to a video feed with music, games, tech, and live categories. Swipe through the feed to load more videos. The bottom bar switches between Home, Shorts, Subscriptions, and Library. Shorts opens a portrait player. Swipe up for the next video or down to go back.

For you mixes videos related to recent watches and searches with uploads from followed channels. Imported YouTube subscriptions are included after account connection.

Search opens from the header and accepts video titles and YouTube video or playlist links. Tap a video to watch it in a queue. The watch screen has a timeline, play and pause, ten-second jumps and fullscreen. You can save or share the video, open its channel and scroll through its description.

Settings offers the same 11 interface languages as the desktop app, the device language, and a thumbnail toggle. Subscriptions, saved videos, and viewing history work locally. Sign in from Subscriptions or Settings to read your YouTube subscriptions. Settings also lets you switch accounts, refresh subscriptions and disconnect.

SponsorBlock skips sponsor, self-promotion and interaction segments. It marks other supported segments below the player. Settings also offers DeArrow titles and thumbnails and Return YouTube Dislike counts.

## Build

Install Rust 1.92 or newer, JDK 17, Android SDK Platform 36, Build Tools 36, Android NDK 29, and Gradle 8.14.3. Set `ANDROID_HOME` and `ANDROID_NDK_HOME` to the SDK and NDK directories.

```powershell
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ferry --version 0.1.0 --locked
cargo ferry check
cargo test --locked --lib
./tools/build-account-apk.ps1
```

Run the commands from `mobile/android`. The script builds the Rust libraries and Android bridge with RustFerry, packages Google Play services with the Android Gradle plugin, and signs `target/google-play/Stream-Tube-Android-0.1.0.apk` with the local RustFerry signing identity.

## YouTube sign-in

Sign-in uses YouTube's TV device flow, like the desktop app. Stream Tube opens youtube.com/activate with a code filled in; pick an account there and allow access. The app keeps the refresh token in its private storage and reads the account name and subscriptions through the TV Innertube client. No Google Cloud project or OAuth client tied to the signing key is needed, so self-signed builds can sign in.

## Updates

Builds from this fork's `android.yml` workflow are published as releases tagged `android-<versionCode>`. The app checks them on start and from Settings, and installs a newer APK through Android's package installer once its SHA-256 matches the digest GitHub reports. Run `tools/setup-android-release.ps1` once to create the release key and the signing secrets.

## Attribution

The Android source is MIT licensed. Slint is used under the [Slint Royalty-free License](https://slint.dev/agreements/slint-royalty-free-license.pdf) and credited in Settings. The release includes dependency licenses.

[![Made with Slint](https://slint.dev/logo/MadeWithSlint-logo-light.svg)](https://slint.dev/)
