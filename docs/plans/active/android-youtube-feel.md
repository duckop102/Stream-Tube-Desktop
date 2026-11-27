# Android: YouTube-like interface, native watch page

Owner: duckop102 fork. Started 2026-11-27. Scope chosen by the user: Home & feed,
watch page with a native player (ExoPlayer), Search & the You tab. Shorts redesign
is out of scope; Shorts must keep playing.

Branding stays Stream Tube's own (no YouTube logo or wordmark); layout, density and
interaction follow the familiar YouTube mobile patterns.

## Workstreams

A. Browse interface (Slint, `src/app/ui.slint`, `src/app/mod.rs` outside the player
   bindings, `src/features/youtube/api.rs`, `src/entities`, `src/features/library.rs`)
   - [x] YouTube dark palette, top bar (logo mark, search, account), scrolling pill chips
   - [x] Full-width video cards: channel avatar, two-line title, meta line, ⋮ action sheet
   - [x] Shorts shelf in the Home feed, skeleton shimmer while loading
   - [x] Bottom bar Home / Shorts / Subscriptions / You; tapping Home again scrolls up and refreshes
   - [x] Search screen with suggestions and recent searches
   - [x] You tab: account header, history and saved rows with View all, settings entry
   - [x] Subscriptions: channel avatar row above the feed

B. Watch page (`src/features/player/**`, `src/features/youtube/watch.rs`,
   `google-play/build.gradle`, `build.rs`, player bindings in `src/app/mod.rs`)
   - [ ] Streams from the visionOS / iOS Innertube clients (HLS, no PO token or JS)
   - [ ] ExoPlayer (Media3) page replacing the WebView; Shorts pager on ExoPlayer
   - [ ] Controls: play/pause, seek with SponsorBlock marks, time, quality, speed, fullscreen, autoplay next
   - [ ] Details: title, meta, channel row with avatar and Subscribe, Like/Dislike/Share/Save pills,
         collapsible description, comments sheet, related videos
   - [ ] Mini player above the bottom bar (swipe down or Back), tap to expand

## Validation

- `cargo test --locked --lib` in `mobile/android` passes on the host
- Java compiles against android-36 and the Media3 jars
- CI `android.yml` builds, signs and publishes; the installed build updates in place
- The user checks the result on the phone

## Log

- 2026-11-27: plan written; B delegated to a subagent with the file ownership above.
- 2026-11-27: A done in code (ui.slint rewrite, avatars, shelf, suggestions, You tab,
  parallel image loader with an avatar cache); host tests pass. Not yet seen on a phone.
