fn main() {
    slint_build::compile("src/app/ui.slint").expect("cannot compile the Stream Tube interface");
    // The player's Java (src/features/player/ui) depends on Media3, so Gradle
    // compiles it with the APK; see google-play/build.gradle.
}
