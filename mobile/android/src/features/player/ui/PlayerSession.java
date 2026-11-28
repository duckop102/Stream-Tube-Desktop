package io.github.shiroksh.streamtube;

import android.graphics.PixelFormat;
import android.animation.ValueAnimator;
import android.app.Activity;
import android.app.Application;
import android.app.Dialog;
import android.app.PendingIntent;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.TextUtils;
import android.view.GestureDetector;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.TextureView;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import androidx.annotation.OptIn;
import androidx.media3.common.AudioAttributes;
import androidx.media3.common.C;
import androidx.media3.common.Format;
import androidx.media3.common.MediaItem;
import androidx.media3.common.MediaMetadata;
import androidx.media3.common.PlaybackException;
import androidx.media3.common.Player;
import androidx.media3.common.TrackSelectionOverride;
import androidx.media3.common.TrackSelectionParameters;
import androidx.media3.common.Tracks;
import androidx.media3.common.VideoSize;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.datasource.DefaultHttpDataSource;
import androidx.media3.exoplayer.ExoPlayer;
import androidx.media3.exoplayer.hls.HlsMediaSource;
import androidx.media3.session.CommandButton;
import androidx.media3.session.MediaSession;
import androidx.media3.session.SessionCommand;
import androidx.media3.session.SessionCommands;
import androidx.media3.session.SessionResult;
import com.google.common.util.concurrent.Futures;
import com.google.common.util.concurrent.ListenableFuture;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.TreeMap;
import org.json.JSONArray;
import org.json.JSONObject;

/** One playback: an ExoPlayer, its dialog window and the watch page or Shorts pager. */
@OptIn(markerClass = UnstableApi.class)
final class PlayerSession implements Application.ActivityLifecycleCallbacks, Player.Listener {
    // The app's colours, after ytm-rs; Settings › Appearance sets them (VideoPlayer.setTheme)
    // and a page takes them when it opens.
    static boolean light;
    static int BACKGROUND = 0xFF0C0A0E;
    static int SURFACE = 0xFF26222A;
    static int MINI_SURFACE = 0xFF1C1820;
    static int TEXT = 0xFFF2F0F5;
    static int SECONDARY = 0xFFB2ACBA;
    static int DIVIDER = 0x1FFFFFFF;
    static int RIPPLE = 0x33FFFFFF;
    static int GLASS = 0x1AFFFFFF;
    static int LINK = 0xFF3EA6FF;
    static int ACCENT = 0xFFFF3048;
    static int ON_ACCENT = Color.WHITE;
    // icons and text over the video, whatever the theme
    static final int ON_VIDEO = 0xFFF2F0F5;
    private static final int MINI_HEIGHT = 64;
    private static final int MINI_MARGIN = 8;
    private static final int MINI_VIDEO_WIDTH = 114;
    private static final int BOTTOM_NAVIGATION = 56;
    private static final float[] SPEEDS = {0.25f, 0.5f, 0.75f, 1f, 1.25f, 1.5f, 1.75f, 2f};
    // the notification's and lock screen's previous and next buttons
    private static final SessionCommand NEXT = new SessionCommand("stream-tube.next", Bundle.EMPTY);
    private static final SessionCommand PREVIOUS = new SessionCommand("stream-tube.previous", Bundle.EMPTY);

    final Activity activity;
    final int session;
    final List<VideoPlayer.Track> queue;
    final String language;
    final JSONObject labels;
    final boolean shorts;
    final Dialog dialog;
    final ExoPlayer player;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final Runnable tick = this::tick;
    // when the video has wanted to play without playing, to say so after a while
    private long waitingSince;
    private boolean stallReported;
    private final Runnable hideChrome = () -> setChromeVisible(false);
    private final FrameLayout root;
    private final FrameLayout videoBox;
    private final VideoFrame videoFrame;
    private final ProgressBar spinner;
    private final LinearLayout errorBox;
    private final TextView errorText;
    private final TextView note;
    private final FrameLayout sheetLayer;
    private final Set<Integer> skipped = new HashSet<>();
    private final int originalOrientation;
    // lets the video go on in the background, with its notification; none for Shorts
    private MediaSession mediaSession;
    private boolean background;

    // watch page
    private Controls controls;
    private WatchPage page;
    private LinearLayout miniBar;
    // the video's picture, blurred, behind the watch page; it drifts slowly while shown
    private ImageView backdrop;
    private View backdropShade;
    private ValueAnimator drift;
    private TextView miniTitle;
    private TextView miniChannel;
    private ImageView miniPlay;
    private TimeBar miniProgress;
    // Shorts
    private TextView shortsTitle;
    private TextView shortsChannel;
    private TextView shortsSave;
    private ImageView shortsSaveIcon;
    private TextView shortsInitial;
    private ImageView shortsAvatar;
    private TimeBar shortsProgress;

    int index;
    private boolean more;
    private boolean loading;
    private boolean mini;
    private boolean fullscreen;
    private boolean autoplay = true;
    private boolean dismissed;
    private boolean live;
    private int insetTop;
    private int insetBottom;
    private int normalUi;
    private long resumeAt;
    private int qualityHeight;
    private float speed = 1f;
    private JSONArray segments;
    private String streamFor = "";

    PlayerSession(Activity activity, int session, List<VideoPlayer.Track> queue, int first,
                  String language, JSONObject labels, boolean shorts, boolean more) {
        this.activity = activity;
        this.session = session;
        this.queue = queue;
        this.index = first;
        this.language = language;
        this.labels = labels;
        this.shorts = shorts;
        this.more = more;
        originalOrientation = activity.getRequestedOrientation();

        player = new ExoPlayer.Builder(activity).build();
        player.setAudioAttributes(new AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).build(), true);
        player.setHandleAudioBecomingNoisy(true);
        // keeps streaming with the screen off
        player.setWakeMode(C.WAKE_MODE_NETWORK);
        player.setRepeatMode(shorts ? Player.REPEAT_MODE_ONE : Player.REPEAT_MODE_OFF);
        player.addListener(this);
        if (!shorts) {
            mediaSession = buildMediaSession();
        }

        root = new FrameLayout(activity) {
            @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
                super.onSizeChanged(width, height, oldWidth, oldHeight);
                post(PlayerSession.this::layout);
            }
        };
        root.setBackgroundColor(shorts ? Color.BLACK : BACKGROUND);
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets bars = insets.getInsets(
                    WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                insetTop = bars.top;
                insetBottom = bars.bottom;
            } else {
                insetTop = insets.getSystemWindowInsetTop();
                insetBottom = insets.getSystemWindowInsetBottom();
            }
            view.post(this::layout);
            return insets;
        });

        videoBox = shorts ? new ShortsGesture(activity, this::move) : new FrameLayout(activity);
        videoBox.setBackgroundColor(Color.BLACK);
        videoFrame = new VideoFrame(activity);
        videoFrame.setFill(shorts);
        TextureView texture = new TextureView(activity);
        videoFrame.addView(texture);
        videoBox.addView(videoFrame, new FrameLayout.LayoutParams(-1, -1));
        player.setVideoTextureView(texture);

        spinner = new ProgressBar(activity);
        spinner.setIndeterminateTintList(ColorStateList.valueOf(Color.WHITE));
        videoBox.addView(spinner, new FrameLayout.LayoutParams(dp(40), dp(40), Gravity.CENTER));

        errorBox = new LinearLayout(activity);
        errorBox.setOrientation(LinearLayout.VERTICAL);
        errorBox.setGravity(Gravity.CENTER);
        errorBox.setPadding(dp(24), 0, dp(24), 0);
        errorBox.setBackgroundColor(Color.BLACK);
        errorText = label("", 14, ON_VIDEO);
        errorText.setGravity(Gravity.CENTER);
        errorText.setMaxLines(3);
        errorText.setEllipsize(TextUtils.TruncateAt.END);
        errorBox.addView(errorText, new LinearLayout.LayoutParams(-2, -2));
        TextView retry = pill(text("Retry"), null, 0);
        retry.setTextColor(Color.BLACK);
        retry.setBackground(rounded(ON_VIDEO, dp(18)));
        retry.setOnClickListener(view -> retry());
        LinearLayout.LayoutParams retryParams = new LinearLayout.LayoutParams(-2, dp(36));
        retryParams.topMargin = dp(14);
        errorBox.addView(retry, retryParams);
        errorBox.setVisibility(View.GONE);
        videoBox.addView(errorBox, new FrameLayout.LayoutParams(-1, -1));

        note = label("", 13, Color.WHITE);
        note.setPadding(dp(10), dp(6), dp(10), dp(6));
        note.setBackground(rounded(0xCC000000, dp(6)));
        note.setVisibility(View.GONE);
        FrameLayout.LayoutParams noteParams = new FrameLayout.LayoutParams(-2, -2, Gravity.TOP | Gravity.END);
        noteParams.setMargins(dp(10), dp(shorts ? 56 : 10), dp(10), 0);

        if (!shorts) {
            buildBackdrop();
        }
        root.addView(videoBox, new FrameLayout.LayoutParams(-1, -1));
        if (shorts) {
            videoBox.addView(note, noteParams);
            buildShorts();
        } else {
            controls = new Controls();
            videoBox.addView(controls, new FrameLayout.LayoutParams(-1, -1));
            videoBox.addView(note, noteParams);
            page = new WatchPage(this);
            root.addView(page.view(), new FrameLayout.LayoutParams(-1, -1));
            buildMiniBar();
        }
        sheetLayer = new FrameLayout(activity);
        sheetLayer.setVisibility(View.GONE);
        root.addView(sheetLayer, new FrameLayout.LayoutParams(-1, -1));

        dialog = new Dialog(activity, android.R.style.Theme_Material_NoActionBar) {
            @Override public void onBackPressed() {
                if (!back()) {
                    super.onBackPressed();
                }
            }
        };
        dialog.setContentView(root);
        dialog.setOnDismissListener(closed -> release());
        if (Build.VERSION.SDK_INT >= 33) {
            BackCallbacks.register(dialog, this);
        }
    }

    void show() {
        dialog.show();
        Window window = dialog.getWindow();
        if (window != null) {
            // the floating mini player's rounded corners show the app behind them
            window.setFormat(PixelFormat.TRANSLUCENT);
            window.setBackgroundDrawable(new ColorDrawable(shorts ? Color.BLACK : BACKGROUND));
            window.setStatusBarColor(Color.BLACK);
            window.setNavigationBarColor(shorts ? Color.BLACK : BACKGROUND);
            View decor = window.getDecorView();
            if (light && !shorts && Build.VERSION.SDK_INT >= 26) {
                decor.setSystemUiVisibility(decor.getSystemUiVisibility()
                    | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
            }
            normalUi = decor.getSystemUiVisibility();
            applyWindow();
        }
        activity.getApplication().registerActivityLifecycleCallbacks(this);
        if (mediaSession != null) {
            PlaybackService.attach(activity, mediaSession);
        }
        watch();
        handler.post(tick);
    }

    void dismiss() {
        if (dismissed) {
            return;
        }
        // the dismiss listener runs a moment later; stop the sound now
        player.pause();
        if (dialog.isShowing()) {
            dialog.dismiss();
        } else {
            release();
        }
    }

    private void release() {
        if (dismissed) {
            return;
        }
        dismissed = true;
        handler.removeCallbacksAndMessages(null);
        activity.getApplication().unregisterActivityLifecycleCallbacks(this);
        player.removeListener(this);
        if (drift != null) {
            drift.cancel();
        }
        if (mediaSession != null) {
            PlaybackService.detach(mediaSession);
            mediaSession.release();
            mediaSession = null;
        }
        player.release();
        activity.setRequestedOrientation(originalOrientation);
        if (VideoPlayer.active == this) {
            VideoPlayer.active = null;
        }
        VideoPlayer.event(session, VideoPlayer.CLOSED, "");
    }

    VideoPlayer.Track track() {
        return queue.get(index);
    }

    // ───────────── playback ─────────────

    private void watch() {
        VideoPlayer.Track track = track();
        player.stop();
        player.clearMediaItems();
        segments = null;
        skipped.clear();
        resumeAt = 0;
        live = false;
        qualityHeight = 0;
        player.setTrackSelectionParameters(player.getTrackSelectionParameters().buildUpon()
            .clearOverridesOfType(C.TRACK_TYPE_VIDEO).build());
        videoFrame.setAspect(shorts ? 9f / 16f : 16f / 9f);
        errorBox.setVisibility(View.GONE);
        spinner.setVisibility(View.VISIBLE);
        if (shorts) {
            shortsTitle.setText(track.title);
            shortsChannel.setText(track.channel);
            View owner = (View) shortsChannel.getParent();
            owner.setVisibility(track.channel.isEmpty() ? View.GONE : View.VISIBLE);
            String name = track.channel.replaceFirst("^@", "").trim();
            shortsInitial.setText(name.isEmpty() ? "" : name.substring(0, name.offsetByCodePoints(0, 1)).toUpperCase());
            RemoteImages.load(shortsAvatar, track.avatar, dp(32));
            shortsProgress.setTimes(0, 0, 0);
            videoBox.setContentDescription(track.title);
        } else {
            page.bind(track, index + 1 < queue.size() || more);
            RemoteImages.loadBlurred(backdrop, RemoteImages.thumbnail(track.id));
            miniTitle.setText(track.title);
            miniChannel.setText(track.channel);
            controls.reset();
            miniProgress.setTimes(0, 0, 0);
        }
        updateSaved(track.id, track.saved);
        VideoPlayer.event(session, VideoPlayer.WATCHED, track.id);
        requestStream();
    }

    private void requestStream() {
        streamFor = track().id;
        VideoPlayer.event(session, VideoPlayer.STREAM, streamFor);
    }

    private void retry() {
        errorBox.setVisibility(View.GONE);
        spinner.setVisibility(View.VISIBLE);
        requestStream();
    }

    void stream(JSONObject packet) {
        if (!streamFor.equals(track().id)) {
            return;
        }
        String url = packet.optString("url", "");
        String agent = packet.optString("agent", "");
        if (packet.has("error") || !url.startsWith("https://") || agent.isEmpty()) {
            showError(packet.optString("error", ""));
            return;
        }
        live = packet.optBoolean("live", false);
        DefaultHttpDataSource.Factory http = new DefaultHttpDataSource.Factory()
            .setUserAgent(agent)
            .setAllowCrossProtocolRedirects(true)
            .setConnectTimeoutMs(15000)
            .setReadTimeoutMs(20000);
        VideoPlayer.Track track = track();
        // what the notification and lock screen show
        MediaMetadata metadata = new MediaMetadata.Builder()
            .setTitle(track.title)
            .setArtist(track.channel)
            .setArtworkUri(Uri.parse("https://i.ytimg.com/vi/" + track.id + "/hqdefault.jpg"))
            .build();
        HlsMediaSource source = new HlsMediaSource.Factory(http)
            .setAllowChunklessPreparation(true)
            .createMediaSource(new MediaItem.Builder().setUri(url).setMediaMetadata(metadata).build());
        if (live) {
            player.setMediaSource(source);
        } else {
            player.setMediaSource(source, resumeAt);
        }
        player.setPlaybackSpeed(speed);
        waitingSince = 0;
        stallReported = false;
        player.prepare();
        player.setPlayWhenReady(true);
    }

    private void showError(String detail) {
        spinner.setVisibility(View.GONE);
        String message = text("Could not play this video");
        String first = detail.isEmpty() ? "" : detail.split("\n")[0];
        errorText.setText(first.isEmpty() ? message : message + "\n" + first);
        errorBox.setVisibility(View.VISIBLE);
        if (controls != null) {
            controls.updatePlaying();
        }
    }

    @Override public void onPlaybackStateChanged(int state) {
        spinner.setVisibility(state == Player.STATE_BUFFERING ? View.VISIBLE : View.GONE);
        if (state == Player.STATE_ENDED && !shorts) {
            if (autoplay && (index + 1 < queue.size() || more)) {
                move(1);
            } else if (controls != null) {
                setChromeVisible(true);
            }
        }
        if (controls != null) {
            controls.updatePlaying();
        }
    }

    @Override public void onIsPlayingChanged(boolean playing) {
        root.setKeepScreenOn(playing);
        if (controls != null) {
            controls.updatePlaying();
            if (playing) {
                scheduleHide();
            }
        }
        updateDrift();
        if (miniPlay != null) {
            miniPlay.setImageDrawable(PlayerIcons.of(playing ? PlayerIcons.PAUSE : PlayerIcons.PLAY, TEXT));
        }
    }

    @Override public void onPlayerError(PlaybackException error) {
        long position = player.getCurrentPosition();
        resumeAt = live ? 0 : Math.max(resumeAt, position);
        String code = error.getErrorCodeName();
        showError(code == null ? "" : code);
    }

    @Override public void onVideoSizeChanged(VideoSize size) {
        if (size.width > 0 && size.height > 0) {
            videoFrame.setAspect(size.width * size.pixelWidthHeightRatio / size.height);
        }
    }

    void togglePlay() {
        if (player.getPlaybackState() == Player.STATE_ENDED) {
            player.seekTo(0);
            player.play();
        } else if (player.getPlaybackState() == Player.STATE_IDLE) {
            if (errorBox.getVisibility() == View.VISIBLE) {
                retry();
            }
        } else if (player.isPlaying() || player.getPlayWhenReady()) {
            player.pause();
        } else {
            player.play();
        }
    }

    private void seekBy(long delta) {
        long duration = player.getDuration();
        long target = Math.max(0, player.getCurrentPosition() + delta);
        if (duration != C.TIME_UNSET && duration > 0) {
            target = Math.min(duration, target);
        }
        player.seekTo(target);
    }

    private void tick() {
        if (dismissed) {
            return;
        }
        long position = player.getCurrentPosition();
        long buffered = player.getBufferedPosition();
        long duration = player.getDuration();
        if (duration == C.TIME_UNSET) {
            duration = 0;
        }
        if (!live && player.getPlaybackState() != Player.STATE_IDLE) {
            resumeAt = position;
        }
        if (controls != null) {
            controls.updateTime(position, buffered, duration);
            miniProgress.setTimes(position, buffered, duration);
        }
        if (shortsProgress != null) {
            shortsProgress.setTimes(position, buffered, duration);
        }
        skipSegments(position, duration);
        reportStall(position, buffered);
        handler.postDelayed(tick, 250);
    }

    private void skipSegments(long position, long duration) {
        if (segments == null || !player.isPlaying()) {
            return;
        }
        for (int i = 0; i < segments.length(); i++) {
            JSONObject segment = segments.optJSONObject(i);
            if (segment == null || !segment.optBoolean("auto_skip") || skipped.contains(i)) {
                continue;
            }
            long start = (long) (segment.optDouble("start", 0) * 1000);
            long end = (long) (segment.optDouble("end", 0) * 1000);
            if (position >= start && position < end - 150 && (duration <= 0 || end < duration)) {
                skipped.add(i);
                player.seekTo(end);
                showNote("SponsorBlock");
                return;
            }
        }
    }

    private void showNote(String message) {
        showNote(message, 2200);
    }

    private void showNote(String message, long millis) {
        note.setText(message);
        note.setVisibility(View.VISIBLE);
        handler.removeCallbacks(hideNote);
        handler.postDelayed(hideNote, millis);
    }

    /** A video that should play but has not for 15 s gets a note with what the player is doing. */
    private void reportStall(long position, long buffered) {
        int state = player.getPlaybackState();
        boolean waiting = player.getPlayWhenReady() && !player.isPlaying()
            && state != Player.STATE_ENDED && errorBox.getVisibility() != View.VISIBLE;
        long now = android.os.SystemClock.elapsedRealtime();
        if (!waiting) {
            waitingSince = 0;
            return;
        }
        if (waitingSince == 0) {
            waitingSince = now;
            return;
        }
        if (stallReported || now - waitingSince < 15000) {
            return;
        }
        stallReported = true;
        String[] names = {"", "idle", "buffering", "ready", "ended"};
        String name = state >= 0 && state < names.length ? names[state] : String.valueOf(state);
        VideoSize size = player.getVideoSize();
        showNote(text("Still loading") + ": " + name + ", " + position / 1000 + "s/" + buffered / 1000
            + "s, " + size.width + "x" + size.height, 8000);
    }

    private final Runnable hideNote = this::hideNote;

    private void hideNote() {
        note.setVisibility(View.GONE);
    }

    void move(int direction) {
        int destination = index + direction;
        if (destination >= 0 && destination < queue.size()) {
            index = destination;
            watch();
        } else if (destination == queue.size() && more && !loading) {
            loading = true;
            if (page != null) {
                page.queueState(true);
            }
            VideoPlayer.event(session, VideoPlayer.MORE, track().id);
        } else if (direction < 0) {
            player.seekTo(0);
        }
    }

    void append(List<VideoPlayer.Track> tracks, boolean hasMore, boolean failed) {
        boolean advance = loading && index + 1 == queue.size();
        queue.addAll(tracks);
        more = hasMore;
        loading = false;
        if (advance && !tracks.isEmpty()) {
            move(1);
        } else if (page != null) {
            page.queueState(index + 1 < queue.size() || more);
        }
        if (failed) {
            Toast.makeText(activity, text("Could not load more videos"), Toast.LENGTH_SHORT).show();
        }
    }

    /** A related video replaces the current queue position and starts. */
    void playRelated(JSONObject item) {
        String id = item.optString("id", "");
        if (!VideoPlayer.VIDEO_ID.matcher(id).matches()) {
            return;
        }
        String channelId = item.optString("channel_id", "");
        VideoPlayer.event(session, VideoPlayer.RELATED, item.toString());
        queue.set(index, new VideoPlayer.Track(id, item.optString("title", ""),
            item.optString("channel", ""), VideoPlayer.CHANNEL_ID.matcher(channelId).matches()));
        watch();
    }

    // ───────────── packets from Rust ─────────────

    void updateSaved(String id, boolean saved) {
        for (VideoPlayer.Track track : queue) {
            if (track.id.equals(id)) {
                track.saved = saved;
            }
        }
        if (!track().id.equals(id)) {
            return;
        }
        if (page != null) {
            page.saved(saved);
        }
        if (shortsSave != null) {
            shortsSave.setText(saved ? text("Saved") : text("Save"));
            shortsSaveIcon.setImageDrawable(PlayerIcons.of(saved ? PlayerIcons.SAVED : PlayerIcons.SAVE, Color.WHITE));
        }
    }

    void extras(JSONObject packet) {
        segments = packet.optJSONArray("segments");
        skipped.clear();
        if (controls != null) {
            controls.timeBar.setSegments(segments);
        }
        if (page != null) {
            page.extras(packet);
            if (!packet.isNull("title") && !packet.optString("title", "").isEmpty()) {
                miniTitle.setText(packet.optString("title"));
            }
        }
    }

    void details(JSONObject packet) {
        if (page != null && !packet.optBoolean("error", false)) {
            page.details(packet);
        }
    }

    void comments(JSONObject packet) {
        if (page != null) {
            page.comments(packet);
        }
    }

    void following(String channel, boolean following) {
        if (page != null) {
            page.following(channel, following);
        }
    }

    // ───────────── actions from the page ─────────────

    void save() {
        VideoPlayer.event(session, VideoPlayer.SAVE, track().id);
    }

    void openChannel(String channelId, String channelName) {
        String id = track().id;
        if (VideoPlayer.CHANNEL_ID.matcher(channelId).matches()) {
            id += "\n" + channelId + "\n" + channelName;
        } else if (!track().channelLink) {
            return;
        }
        VideoPlayer.event(session, VideoPlayer.CHANNEL, id);
        if (shorts) {
            dismiss();
        } else {
            minimize();
        }
    }

    void share() {
        Intent share = new Intent(Intent.ACTION_SEND).setType("text/plain")
            .putExtra(Intent.EXTRA_TEXT, "https://www.youtube.com/watch?v=" + track().id);
        try {
            activity.startActivity(Intent.createChooser(share, text("Share video")));
        } catch (ActivityNotFoundException unavailable) {
            Toast.makeText(activity, text("No app available"), Toast.LENGTH_SHORT).show();
        }
    }

    // ───────────── window modes ─────────────

    boolean back() {
        if (sheetLayer.getVisibility() == View.VISIBLE) {
            closeSheet();
            return true;
        }
        if (shorts || mini) {
            return false;
        }
        if (page.back()) {
            return true;
        }
        if (fullscreen || isWide()) {
            setFullscreen(false);
            return true;
        }
        minimize();
        return true;
    }

    void minimize() {
        if (mini || shorts || dismissed) {
            return;
        }
        closeSheet();
        page.back();
        mini = true;
        fullscreen = false;
        activity.setRequestedOrientation(originalOrientation);
        setChromeVisible(false);
        applyWindow();
        layout();
    }

    void expand() {
        if (!mini || dismissed) {
            return;
        }
        mini = false;
        applyWindow();
        layout();
        setChromeVisible(true);
    }

    void setFullscreen(boolean value) {
        fullscreen = value;
        activity.setRequestedOrientation(value
            ? ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            : ActivityInfo.SCREEN_ORIENTATION_PORTRAIT);
        layout();
    }

    private boolean isWide() {
        return !mini && (fullscreen || root.getWidth() > root.getHeight());
    }

    private void applyWindow() {
        Window window = dialog.getWindow();
        if (window == null) {
            return;
        }
        WindowManager.LayoutParams attributes = window.getAttributes();
        attributes.width = ViewGroup.LayoutParams.MATCH_PARENT;
        int pass = WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL;
        if (mini) {
            // a card floating over the app, clear of its bottom navigation, as in ytm-rs
            int screen = activity.getWindow().getDecorView().getWidth();
            attributes.width = screen > dp(2 * MINI_MARGIN) ? screen - dp(2 * MINI_MARGIN)
                : ViewGroup.LayoutParams.MATCH_PARENT;
            attributes.height = dp(MINI_HEIGHT);
            attributes.gravity = Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL;
            attributes.y = miniOffset() + dp(MINI_MARGIN);
            attributes.flags |= pass;
            attributes.flags &= ~WindowManager.LayoutParams.FLAG_DIM_BEHIND;
        } else {
            attributes.height = ViewGroup.LayoutParams.MATCH_PARENT;
            attributes.gravity = Gravity.CENTER;
            attributes.y = 0;
            attributes.flags &= ~pass;
        }
        if (Build.VERSION.SDK_INT >= 30) {
            attributes.setFitInsetsTypes(0);
        }
        if (Build.VERSION.SDK_INT >= 28) {
            attributes.layoutInDisplayCutoutMode =
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        }
        window.setAttributes(attributes);
        if (mini) {
            window.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
            GradientDrawable card = rounded(MINI_SURFACE, dp(16));
            card.setStroke(Math.max(1, dp(1) / 2), DIVIDER);
            root.setBackground(card);
            root.setClipToOutline(true);
        } else {
            window.setBackgroundDrawable(new ColorDrawable(shorts ? Color.BLACK : BACKGROUND));
            root.setClipToOutline(false);
            root.setBackgroundColor(shorts ? Color.BLACK : BACKGROUND);
        }
    }

    /** The mini player sits on the app's bottom navigation bar, above the system bar. */
    private int miniOffset() {
        int navigation = 0;
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsets insets = activity.getWindow().getDecorView().getRootWindowInsets();
            if (insets != null) {
                navigation = insets.getInsets(WindowInsets.Type.navigationBars()).bottom;
            }
        }
        return navigation + dp(BOTTOM_NAVIGATION);
    }

    private void layout() {
        int width = root.getWidth();
        int height = root.getHeight();
        if (width == 0 || height == 0 || dismissed) {
            return;
        }
        if (shorts) {
            immersive(false);
            return;
        }
        boolean wide = isWide();
        int videoHeight = width * 9 / 16;
        FrameLayout.LayoutParams video = (FrameLayout.LayoutParams) videoBox.getLayoutParams();
        FrameLayout.LayoutParams below = (FrameLayout.LayoutParams) page.view().getLayoutParams();
        if (mini) {
            video.width = dp(MINI_VIDEO_WIDTH);
            video.height = ViewGroup.LayoutParams.MATCH_PARENT;
            video.topMargin = 0;
        } else if (wide) {
            video.width = ViewGroup.LayoutParams.MATCH_PARENT;
            video.height = ViewGroup.LayoutParams.MATCH_PARENT;
            video.topMargin = 0;
        } else {
            video.width = ViewGroup.LayoutParams.MATCH_PARENT;
            video.height = videoHeight;
            video.topMargin = insetTop;
            below.topMargin = insetTop + videoHeight;
            below.bottomMargin = insetBottom;
            page.view().setLayoutParams(below);
        }
        videoBox.setLayoutParams(video);
        page.view().setVisibility(!mini && !wide ? View.VISIBLE : View.GONE);
        backdrop.setVisibility(!mini && !wide ? View.VISIBLE : View.GONE);
        backdropShade.setVisibility(backdrop.getVisibility());
        updateDrift();
        miniBar.setVisibility(mini ? View.VISIBLE : View.GONE);
        miniProgress.setVisibility(mini ? View.VISIBLE : View.GONE);
        controls.setMode(mini, wide);
        immersive(wide);
    }

    private void immersive(boolean on) {
        Window window = dialog.getWindow();
        if (window == null) {
            return;
        }
        View decor = window.getDecorView();
        int flags = on
            ? normalUi | View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
            : normalUi;
        if (decor.getSystemUiVisibility() != flags) {
            decor.setSystemUiVisibility(flags);
        }
    }

    private void setChromeVisible(boolean visible) {
        if (controls != null) {
            controls.setChromeVisible(visible && !mini);
        }
    }

    private void scheduleHide() {
        handler.removeCallbacks(hideChrome);
        if (player.isPlaying()) {
            handler.postDelayed(hideChrome, 3000);
        }
    }

    // ───────────── settings sheet ─────────────

    private static final class Option {
        final String label;
        final String value;
        final boolean selected;
        final Runnable action;

        Option(String label, String value, boolean selected, Runnable action) {
            this.label = label;
            this.value = value;
            this.selected = selected;
            this.action = action;
        }
    }

    void openSettings() {
        List<Option> options = new ArrayList<>();
        options.add(new Option(text("Quality"), qualityLabel(), false, this::openQuality));
        options.add(new Option(text("Playback speed"), speedLabel(speed), false, this::openSpeed));
        options.add(new Option(text("Autoplay"), autoplay ? text("On") : text("Off"), false, () -> {
            autoplay = !autoplay;
            closeSheet();
        }));
        showSheet(null, options);
    }

    private String qualityLabel() {
        if (qualityHeight > 0) {
            return qualityHeight + "p";
        }
        Format format = player.getVideoFormat();
        return format != null && format.height > 0
            ? text("Auto") + " (" + format.height + "p)" : text("Auto");
    }

    private static String speedLabel(float value) {
        if (value == 1f) {
            return "1x";
        }
        String number = value == (int) value ? String.valueOf((int) value) : String.valueOf(value);
        return number + "x";
    }

    private void openQuality() {
        TreeMap<Integer, TrackSelectionOverride> heights = new TreeMap<>((a, b) -> b - a);
        for (Tracks.Group group : player.getCurrentTracks().getGroups()) {
            if (group.getType() != C.TRACK_TYPE_VIDEO) {
                continue;
            }
            for (int i = 0; i < group.length; i++) {
                Format format = group.getTrackFormat(i);
                if (!group.isTrackSupported(i) || format.height <= 0) {
                    continue;
                }
                TrackSelectionOverride current = heights.get(format.height);
                if (current == null || bitrate(current) < format.bitrate) {
                    heights.put(format.height, new TrackSelectionOverride(group.getMediaTrackGroup(), i));
                }
            }
        }
        List<Option> options = new ArrayList<>();
        options.add(new Option(text("Auto"), "", qualityHeight == 0, () -> {
            qualityHeight = 0;
            player.setTrackSelectionParameters(player.getTrackSelectionParameters().buildUpon()
                .clearOverridesOfType(C.TRACK_TYPE_VIDEO).build());
            closeSheet();
        }));
        for (java.util.Map.Entry<Integer, TrackSelectionOverride> entry : heights.entrySet()) {
            int height = entry.getKey();
            TrackSelectionOverride override = entry.getValue();
            options.add(new Option(height + "p", "", qualityHeight == height, () -> {
                qualityHeight = height;
                TrackSelectionParameters parameters = player.getTrackSelectionParameters().buildUpon()
                    .setOverrideForType(override).build();
                player.setTrackSelectionParameters(parameters);
                closeSheet();
            }));
        }
        showSheet(text("Quality"), options);
    }

    private static int bitrate(TrackSelectionOverride override) {
        return override.mediaTrackGroup.getFormat(override.trackIndices.get(0)).bitrate;
    }

    private void openSpeed() {
        List<Option> options = new ArrayList<>();
        for (float value : SPEEDS) {
            options.add(new Option(value == 1f ? text("Normal") : speedLabel(value), "", speed == value, () -> {
                speed = value;
                player.setPlaybackSpeed(value);
                closeSheet();
            }));
        }
        showSheet(text("Playback speed"), options);
    }

    private void showSheet(String title, List<Option> options) {
        sheetLayer.removeAllViews();
        View scrim = new View(activity);
        scrim.setBackgroundColor(0x99000000);
        scrim.setOnClickListener(view -> closeSheet());
        sheetLayer.addView(scrim, new FrameLayout.LayoutParams(-1, -1));
        LinearLayout panel = new LinearLayout(activity);
        panel.setOrientation(LinearLayout.VERTICAL);
        panel.setClickable(true);
        panel.setPadding(0, dp(8), 0, dp(12) + (isWide() ? 0 : insetBottom));
        GradientDrawable background = new GradientDrawable();
        background.setColor(MINI_SURFACE);
        float radius = dp(14);
        background.setCornerRadii(new float[]{radius, radius, radius, radius, 0, 0, 0, 0});
        panel.setBackground(background);
        View handle = new View(activity);
        handle.setBackground(rounded(0xFF5F5F5F, dp(2)));
        LinearLayout.LayoutParams handleParams = new LinearLayout.LayoutParams(dp(36), dp(4));
        handleParams.gravity = Gravity.CENTER_HORIZONTAL;
        handleParams.bottomMargin = dp(8);
        panel.addView(handle, handleParams);
        if (title != null) {
            TextView heading = label(title, 16, TEXT);
            heading.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
            heading.setPadding(dp(20), dp(6), dp(20), dp(10));
            panel.addView(heading);
        }
        LinearLayout list = new LinearLayout(activity);
        list.setOrientation(LinearLayout.VERTICAL);
        for (Option option : options) {
            LinearLayout row = new LinearLayout(activity);
            row.setGravity(Gravity.CENTER_VERTICAL);
            row.setPadding(dp(20), 0, dp(20), 0);
            row.setBackground(ripple());
            TextView name = label(option.label, 15, TEXT);
            if (option.selected) {
                name.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
                name.setText("✓  " + option.label);
            }
            row.addView(name, new LinearLayout.LayoutParams(0, -2, 1));
            if (!option.value.isEmpty()) {
                TextView value = label(option.value + "  ›", 14, SECONDARY);
                row.addView(value, new LinearLayout.LayoutParams(-2, -2));
            }
            row.setOnClickListener(view -> option.action.run());
            list.addView(row, new LinearLayout.LayoutParams(-1, dp(48)));
        }
        ScrollView scroll = new ScrollView(activity);
        scroll.addView(list);
        panel.addView(scroll, new LinearLayout.LayoutParams(-1, -2));
        int maxHeight = (int) (Math.max(root.getHeight(), dp(300)) * 0.7f);
        scroll.measure(View.MeasureSpec.makeMeasureSpec(Math.max(1, root.getWidth()), View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        if (scroll.getMeasuredHeight() > maxHeight) {
            scroll.getLayoutParams().height = maxHeight;
        }
        FrameLayout.LayoutParams panelParams = new FrameLayout.LayoutParams(
            isWide() ? Math.min(root.getWidth(), dp(480)) : -1, -2, Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        sheetLayer.addView(panel, panelParams);
        sheetLayer.setVisibility(View.VISIBLE);
        handler.removeCallbacks(hideChrome);
    }

    void closeSheet() {
        sheetLayer.setVisibility(View.GONE);
        sheetLayer.removeAllViews();
    }

    // ───────────── building ─────────────

    private void buildMiniBar() {
        miniBar = new LinearLayout(activity);
        miniBar.setGravity(Gravity.CENTER_VERTICAL);
        miniBar.setPadding(dp(12), 0, dp(2), 0);
        miniBar.setOnClickListener(view -> expand());
        // a swipe up opens the watch page, a swipe down closes the video
        GestureDetector swipes = new GestureDetector(activity, new GestureDetector.SimpleOnGestureListener() {
            @Override public boolean onFling(MotionEvent start, MotionEvent end, float velocityX, float velocityY) {
                if (Math.abs(velocityY) < Math.abs(velocityX) || Math.abs(velocityY) < dp(400)) {
                    return false;
                }
                if (velocityY < 0) {
                    expand();
                } else {
                    dismiss();
                }
                return true;
            }
        });
        miniBar.setOnTouchListener((view, event) -> {
            swipes.onTouchEvent(event);
            return false;
        });
        LinearLayout text = new LinearLayout(activity);
        text.setOrientation(LinearLayout.VERTICAL);
        miniTitle = label("", 14, TEXT);
        miniTitle.setSingleLine(true);
        miniTitle.setEllipsize(TextUtils.TruncateAt.END);
        miniChannel = label("", 12, SECONDARY);
        miniChannel.setSingleLine(true);
        miniChannel.setEllipsize(TextUtils.TruncateAt.END);
        text.addView(miniTitle);
        text.addView(miniChannel);
        miniBar.addView(text, new LinearLayout.LayoutParams(0, -2, 1));
        miniPlay = iconButton(PlayerIcons.PLAY, 28, text("Play"));
        miniPlay.setOnClickListener(view -> togglePlay());
        miniBar.addView(miniPlay, new LinearLayout.LayoutParams(dp(44), dp(48)));
        ImageView miniNext = iconButton(PlayerIcons.NEXT, 26, text("Next video"));
        miniNext.setOnClickListener(view -> skip(1));
        miniBar.addView(miniNext, new LinearLayout.LayoutParams(dp(44), dp(48)));
        ImageView close = iconButton(PlayerIcons.CLOSE, 24, text("Close"));
        close.setOnClickListener(view -> dismiss());
        miniBar.addView(close, new LinearLayout.LayoutParams(dp(44), dp(48)));
        FrameLayout.LayoutParams barParams = new FrameLayout.LayoutParams(-1, -1);
        barParams.leftMargin = dp(MINI_VIDEO_WIDTH);
        miniBar.setVisibility(View.GONE);
        root.addView(miniBar, barParams);
        miniProgress = new TimeBar(activity);
        miniProgress.setExpanded(false);
        miniProgress.setAlignBottom(true);
        miniProgress.setVisibility(View.GONE);
        root.addView(miniProgress, new FrameLayout.LayoutParams(-1, dp(2), Gravity.BOTTOM));
    }

    private void buildBackdrop() {
        backdrop = new ImageView(activity);
        backdrop.setScaleType(ImageView.ScaleType.CENTER_CROP);
        backdrop.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        root.addView(backdrop, new FrameLayout.LayoutParams(-1, -1));
        // the picture shows most near the video and fades into the page's colour below
        backdropShade = new View(activity);
        int base = BACKGROUND & 0x00FFFFFF;
        backdropShade.setBackground(new GradientDrawable(GradientDrawable.Orientation.TOP_BOTTOM,
            light ? new int[]{base | 0x99000000, base | 0xE0000000, base | 0xF5000000}
                : new int[]{base | 0x73000000, base | 0xCC000000, base | 0xF0000000}));
        root.addView(backdropShade, new FrameLayout.LayoutParams(-1, -1));
    }

    /** The backdrop drifts and breathes while the watch page shows and the video plays. */
    private void updateDrift() {
        boolean moving = backdrop != null && !dismissed && !background
            && backdrop.getVisibility() == View.VISIBLE && player.isPlaying();
        if (!moving) {
            if (drift != null && drift.isStarted()) {
                drift.pause();
            }
            return;
        }
        if (drift == null) {
            drift = ValueAnimator.ofFloat(0f, 1f);
            drift.setDuration(18000);
            drift.setRepeatCount(ValueAnimator.INFINITE);
            drift.setRepeatMode(ValueAnimator.REVERSE);
            drift.setInterpolator(new android.view.animation.AccelerateDecelerateInterpolator());
            drift.addUpdateListener(animation -> {
                float f = (float) animation.getAnimatedValue();
                float scale = 1.12f + 0.1f * f;
                backdrop.setScaleX(scale);
                backdrop.setScaleY(scale);
                backdrop.setTranslationX(dp(14) * (f - 0.5f) * 2f);
                backdrop.setRotation(2f * (f - 0.5f));
            });
            drift.start();
        } else if (drift.isPaused()) {
            drift.resume();
        } else if (!drift.isStarted()) {
            drift.start();
        }
    }

    private void buildShorts() {
        View tap = new View(activity);
        tap.setOnClickListener(view -> togglePlay());
        videoBox.addView(tap, 1, new FrameLayout.LayoutParams(-1, -1));

        ImageView back = videoButton(PlayerIcons.BACK, 26, text("Back"));
        back.setOnClickListener(view -> dismiss());
        FrameLayout.LayoutParams backParams = new FrameLayout.LayoutParams(dp(48), dp(48), Gravity.TOP | Gravity.START);
        backParams.setMargins(dp(4), dp(28), 0, 0);
        videoBox.addView(back, backParams);

        // as on YouTube: channel and title at the bottom left, actions in a column on the right
        View shade = new View(activity);
        shade.setBackground(new GradientDrawable(GradientDrawable.Orientation.BOTTOM_TOP,
            new int[]{0xB3000000, 0x00000000}));
        videoBox.addView(shade, new FrameLayout.LayoutParams(-1, dp(240), Gravity.BOTTOM));

        LinearLayout info = new LinearLayout(activity);
        info.setOrientation(LinearLayout.VERTICAL);
        info.setPadding(dp(14), 0, dp(80), dp(22));
        LinearLayout owner = new LinearLayout(activity);
        owner.setGravity(Gravity.CENTER_VERTICAL);
        owner.setOnClickListener(view -> openChannel("", ""));
        FrameLayout picture = new FrameLayout(activity);
        GradientDrawable oval = new GradientDrawable();
        oval.setShape(GradientDrawable.OVAL);
        oval.setColor(0xFF3F3F3F);
        picture.setBackground(oval);
        picture.setOutlineProvider(android.view.ViewOutlineProvider.BACKGROUND);
        picture.setClipToOutline(true);
        shortsInitial = label("", 14, Color.WHITE);
        shortsInitial.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        shortsInitial.setGravity(Gravity.CENTER);
        picture.addView(shortsInitial, new FrameLayout.LayoutParams(-1, -1));
        shortsAvatar = new ImageView(activity);
        shortsAvatar.setScaleType(ImageView.ScaleType.CENTER_CROP);
        picture.addView(shortsAvatar, new FrameLayout.LayoutParams(-1, -1));
        owner.addView(picture, new LinearLayout.LayoutParams(dp(32), dp(32)));
        shortsChannel = label("", 14, Color.WHITE);
        shortsChannel.setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL));
        shortsChannel.setSingleLine(true);
        shortsChannel.setEllipsize(TextUtils.TruncateAt.END);
        LinearLayout.LayoutParams channelParams = new LinearLayout.LayoutParams(-2, -2);
        channelParams.setMarginStart(dp(10));
        owner.addView(shortsChannel, channelParams);
        info.addView(owner, new LinearLayout.LayoutParams(-2, dp(40)));
        shortsTitle = label("", 15, Color.WHITE);
        shortsTitle.setMaxLines(2);
        shortsTitle.setEllipsize(TextUtils.TruncateAt.END);
        shortsTitle.setShadowLayer(dp(2), 0, 0, 0x80000000);
        LinearLayout.LayoutParams titleParams = new LinearLayout.LayoutParams(-1, -2);
        titleParams.topMargin = dp(8);
        info.addView(shortsTitle, titleParams);
        videoBox.addView(info, new FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM));

        LinearLayout column = new LinearLayout(activity);
        column.setOrientation(LinearLayout.VERTICAL);
        column.setGravity(Gravity.CENTER_HORIZONTAL);
        column.setPadding(0, 0, 0, dp(24));
        shortsSaveIcon = new ImageView(activity);
        shortsSave = label(text("Save"), 12, Color.WHITE);
        column.addView(shortsAction(shortsSaveIcon, PlayerIcons.SAVE, shortsSave, view -> save()));
        ImageView shareIcon = new ImageView(activity);
        TextView share = label(text("Share"), 12, Color.WHITE);
        View shareAction = shortsAction(shareIcon, PlayerIcons.SHARE, share, view -> share());
        LinearLayout.LayoutParams shareParams = new LinearLayout.LayoutParams(-2, -2);
        shareParams.topMargin = dp(18);
        column.addView(shareAction, shareParams);
        FrameLayout.LayoutParams columnParams = new FrameLayout.LayoutParams(dp(72), -2, Gravity.BOTTOM | Gravity.END);
        columnParams.setMarginEnd(dp(4));
        videoBox.addView(column, columnParams);

        shortsProgress = new TimeBar(activity);
        shortsProgress.setExpanded(false);
        shortsProgress.setAlignBottom(true);
        videoBox.addView(shortsProgress, new FrameLayout.LayoutParams(-1, dp(2), Gravity.BOTTOM));
    }

    /** A round button with its label underneath, for the Shorts action column. */
    private View shortsAction(ImageView icon, String path, TextView label, View.OnClickListener action) {
        LinearLayout item = new LinearLayout(activity);
        item.setOrientation(LinearLayout.VERTICAL);
        item.setGravity(Gravity.CENTER_HORIZONTAL);
        item.setOnClickListener(action);
        item.setContentDescription(label.getText());
        icon.setImageDrawable(PlayerIcons.of(path, Color.WHITE));
        icon.setScaleType(ImageView.ScaleType.FIT_CENTER);
        icon.setPadding(dp(11), dp(11), dp(11), dp(11));
        GradientDrawable oval = new GradientDrawable();
        oval.setShape(GradientDrawable.OVAL);
        oval.setColor(0x40FFFFFF);
        icon.setBackground(new android.graphics.drawable.RippleDrawable(ColorStateList.valueOf(0x40FFFFFF), oval, null));
        item.addView(icon, new LinearLayout.LayoutParams(dp(48), dp(48)));
        label.setGravity(Gravity.CENTER);
        label.setShadowLayer(dp(2), 0, 0, 0x80000000);
        LinearLayout.LayoutParams labelParams = new LinearLayout.LayoutParams(-2, -2);
        labelParams.topMargin = dp(4);
        item.addView(label, labelParams);
        return item;
    }

    // ───────────── view helpers (shared with WatchPage) ─────────────

    String text(String source) {
        return labels.optString(source, source);
    }

    int dp(float value) {
        return Math.round(activity.getResources().getDisplayMetrics().density * value);
    }

    TextView label(String value, float size, int color) {
        TextView view = new TextView(activity);
        view.setText(value);
        view.setTextSize(size);
        view.setTextColor(color);
        return view;
    }

    GradientDrawable rounded(int color, float radius) {
        GradientDrawable fill = new GradientDrawable();
        fill.setColor(color);
        fill.setCornerRadius(radius);
        return fill;
    }

    android.graphics.drawable.Drawable ripple() {
        return new android.graphics.drawable.RippleDrawable(ColorStateList.valueOf(RIPPLE), null,
            new ColorDrawable(Color.WHITE));
    }

    android.graphics.drawable.Drawable icon(String path, int sizeDp) {
        android.graphics.drawable.Drawable drawable = PlayerIcons.of(path, TEXT);
        drawable.setBounds(0, 0, dp(sizeDp), dp(sizeDp));
        return drawable;
    }

    /** A rounded pill over the page, with an optional leading icon. */
    TextView pill(String value, String iconPath, int iconDp) {
        TextView view = label(value, 14, TEXT);
        view.setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL));
        view.setGravity(Gravity.CENTER_VERTICAL);
        view.setSingleLine(true);
        view.setPadding(dp(14), 0, dp(14), 0);
        view.setBackground(new android.graphics.drawable.RippleDrawable(ColorStateList.valueOf(RIPPLE),
            rounded(GLASS, dp(18)), null));
        if (iconPath != null) {
            view.setCompoundDrawablesRelative(icon(iconPath, iconDp), null, null, null);
            view.setCompoundDrawablePadding(dp(6));
        }
        return view;
    }

    ImageView iconButton(String path, int sizeDp, String description) {
        return iconButton(path, sizeDp, description, TEXT);
    }

    /** An icon button over the video: light whatever the theme. */
    ImageView videoButton(String path, int sizeDp, String description) {
        return iconButton(path, sizeDp, description, ON_VIDEO);
    }

    ImageView iconButton(String path, int sizeDp, String description, int tint) {
        ImageView view = new ImageView(activity);
        view.setImageDrawable(PlayerIcons.of(path, tint));
        int padding = Math.max(0, (dp(48) - dp(sizeDp)) / 2);
        view.setPadding(padding, padding, padding, padding);
        view.setScaleType(ImageView.ScaleType.FIT_CENTER);
        view.setContentDescription(description);
        view.setBackground(new android.graphics.drawable.RippleDrawable(
            ColorStateList.valueOf(tint == ON_VIDEO ? 0x33FFFFFF : RIPPLE), null, null));
        return view;
    }

    static void applyTheme(boolean isLight, int accent, int onAccent) {
        light = isLight;
        BACKGROUND = isLight ? 0xFFF6F4F8 : 0xFF0C0A0E;
        SURFACE = isLight ? 0xFFE9E5EE : 0xFF26222A;
        MINI_SURFACE = isLight ? 0xFFFCFBFD : 0xFF1C1820;
        TEXT = isLight ? 0xFF16141A : 0xFFF2F0F5;
        SECONDARY = isLight ? 0xFF56525E : 0xFFB2ACBA;
        DIVIDER = isLight ? 0x2216141A : 0x1FFFFFFF;
        RIPPLE = isLight ? 0x2216141A : 0x33FFFFFF;
        GLASS = isLight ? 0x1216141A : 0x1FFFFFFF;
        LINK = isLight ? 0xFF065FD4 : 0xFF3EA6FF;
        ACCENT = accent;
        ON_ACCENT = onAccent;
    }

    // ───────────── lifecycle ─────────────

    @Override public void onActivityStopped(Activity host) {
        if (host != activity) {
            return;
        }
        if (mediaSession == null) {
            player.pause();
            return;
        }
        // the video goes on as sound; a small picture is enough until the app is back
        background = true;
        updateDrift();
        player.setTrackSelectionParameters(player.getTrackSelectionParameters().buildUpon()
            .setMaxVideoSize(426, 240).build());
    }

    @Override public void onActivityStarted(Activity host) {
        if (host == activity && background) {
            background = false;
            updateDrift();
            player.setTrackSelectionParameters(player.getTrackSelectionParameters().buildUpon()
                .clearVideoSizeConstraints().build());
        }
    }

    @Override public void onActivityDestroyed(Activity host) {
        if (host == activity) {
            dismiss();
        }
    }

    @Override public void onActivityCreated(Activity host, Bundle saved) {}
    @Override public void onActivityResumed(Activity host) {}
    @Override public void onActivityPaused(Activity host) {}
    @Override public void onActivitySaveInstanceState(Activity host, Bundle out) {}

    // ───────────── background playback ─────────────

    private MediaSession buildMediaSession() {
        List<CommandButton> buttons = new ArrayList<>();
        buttons.add(new CommandButton.Builder(CommandButton.ICON_PREVIOUS)
            .setSessionCommand(PREVIOUS)
            .setDisplayName(text("Previous"))
            .setSlots(CommandButton.SLOT_BACK)
            .build());
        buttons.add(new CommandButton.Builder(CommandButton.ICON_NEXT)
            .setSessionCommand(NEXT)
            .setDisplayName(text("Next"))
            .setSlots(CommandButton.SLOT_FORWARD)
            .build());
        MediaSession.Builder builder = new MediaSession.Builder(activity, player)
            .setId("stream-tube-" + session)
            .setMediaButtonPreferences(buttons)
            .setCallback(new SessionCallback());
        // tapping the notification brings the app back
        Intent open = activity.getPackageManager().getLaunchIntentForPackage(activity.getPackageName());
        if (open != null) {
            open.setFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED);
            builder.setSessionActivity(PendingIntent.getActivity(activity, 0, open,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT));
        }
        try {
            return builder.build();
        } catch (RuntimeException failed) {
            // the video plays while the app is open
            return null;
        }
    }

    /** Previous or next from the notification, lock screen or a headset. */
    private void skip(int direction) {
        handler.post(() -> {
            if (dismissed) {
                return;
            }
            if (direction < 0 && (index == 0 || player.getCurrentPosition() > 3000)) {
                player.seekTo(0);
            } else {
                move(direction);
            }
        });
    }

    private final class SessionCallback implements MediaSession.Callback {
        private MediaSession.ConnectionResult accept(MediaSession owner, MediaSession.ControllerInfo controller) {
            SessionCommands commands = MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
                .add(NEXT)
                .add(PREVIOUS)
                .build();
            return new MediaSession.ConnectionResult.AcceptedResultBuilder(owner, controller)
                .setAvailableSessionCommands(commands)
                .build();
        }

        @Override public MediaSession.ConnectionResult onConnect(MediaSession owner,
                                                                MediaSession.ControllerInfo controller) {
            return accept(owner, controller);
        }

        @Override public ListenableFuture<MediaSession.ConnectionResult> onConnectAsync(
                MediaSession owner, MediaSession.ControllerInfo controller) {
            return Futures.immediateFuture(accept(owner, controller));
        }

        @Override public ListenableFuture<SessionResult> onCustomCommand(MediaSession owner,
                MediaSession.ControllerInfo controller, SessionCommand command, Bundle args) {
            if (NEXT.customAction.equals(command.customAction)) {
                skip(1);
            } else if (PREVIOUS.customAction.equals(command.customAction)) {
                skip(-1);
            }
            return Futures.immediateFuture(new SessionResult(SessionResult.RESULT_SUCCESS));
        }

        @Override public boolean onMediaButtonEvent(MediaSession owner,
                MediaSession.ControllerInfo controller, Intent intent) {
            KeyEvent key = intent.getParcelableExtra(Intent.EXTRA_KEY_EVENT);
            if (key == null) {
                return false;
            }
            int code = key.getKeyCode();
            if (code != KeyEvent.KEYCODE_MEDIA_NEXT && code != KeyEvent.KEYCODE_MEDIA_PREVIOUS) {
                return false;
            }
            if (key.getAction() == KeyEvent.ACTION_DOWN && key.getRepeatCount() == 0) {
                skip(code == KeyEvent.KEYCODE_MEDIA_NEXT ? 1 : -1);
            }
            return true;
        }
    }

    // ───────────── controls over the video ─────────────

    private final class Controls extends FrameLayout {
        final TimeBar timeBar;
        private final FrameLayout chrome;
        private final ImageView play;
        private final ImageView previous;
        private final ImageView next;
        private final ImageView fullscreenButton;
        private final TextView time;
        private final TextView seekNote;
        private final LinearLayout bottomRow;
        private final GestureDetector gestures;
        private boolean chromeVisible = true;
        private boolean wide;
        private boolean dragging;
        private float downY;
        private float downX;

        Controls() {
            super(activity);
            chrome = new FrameLayout(activity);
            chrome.setBackgroundColor(0x66000000);
            addView(chrome, new LayoutParams(-1, -1));

            ImageView minimize = videoButton(PlayerIcons.CHEVRON_DOWN, 28, text("Minimize"));
            minimize.setOnClickListener(view -> {
                if (wide) {
                    setFullscreen(false);
                } else {
                    minimize();
                }
            });
            chrome.addView(minimize, new LayoutParams(dp(48), dp(48), Gravity.TOP | Gravity.START));
            ImageView settings = videoButton(PlayerIcons.SETTINGS, 24, text("Settings"));
            settings.setOnClickListener(view -> openSettings());
            chrome.addView(settings, new LayoutParams(dp(48), dp(48), Gravity.TOP | Gravity.END));

            LinearLayout center = new LinearLayout(activity);
            center.setGravity(Gravity.CENTER);
            previous = videoButton(PlayerIcons.PREVIOUS, 32, text("Previous video"));
            previous.setOnClickListener(view -> move(-1));
            play = videoButton(PlayerIcons.PAUSE, 44, text("Pause"));
            play.setOnClickListener(view -> {
                togglePlay();
                scheduleHide();
            });
            next = videoButton(PlayerIcons.NEXT, 32, text("Next video"));
            next.setOnClickListener(view -> move(1));
            center.addView(previous, new LinearLayout.LayoutParams(dp(56), dp(56)));
            LinearLayout.LayoutParams playParams = new LinearLayout.LayoutParams(dp(72), dp(72));
            playParams.setMargins(dp(28), 0, dp(28), 0);
            center.addView(play, playParams);
            center.addView(next, new LinearLayout.LayoutParams(dp(56), dp(56)));
            chrome.addView(center, new LayoutParams(-2, -2, Gravity.CENTER));

            bottomRow = new LinearLayout(activity);
            bottomRow.setGravity(Gravity.CENTER_VERTICAL);
            bottomRow.setPadding(dp(12), 0, 0, 0);
            time = label("", 12, Color.WHITE);
            bottomRow.addView(time, new LinearLayout.LayoutParams(0, -2, 1));
            fullscreenButton = videoButton(PlayerIcons.FULLSCREEN, 24, text("Fullscreen"));
            fullscreenButton.setOnClickListener(view -> setFullscreen(!wide));
            bottomRow.addView(fullscreenButton, new LinearLayout.LayoutParams(dp(44), dp(44)));
            chrome.addView(bottomRow, new LayoutParams(-1, dp(44), Gravity.BOTTOM));

            seekNote = label("", 14, Color.WHITE);
            seekNote.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
            seekNote.setPadding(dp(14), dp(8), dp(14), dp(8));
            seekNote.setBackground(rounded(0x66000000, dp(20)));
            seekNote.setVisibility(GONE);
            addView(seekNote, new LayoutParams(-2, -2, Gravity.CENTER_VERTICAL | Gravity.START));

            timeBar = new TimeBar(activity);
            timeBar.setListener(new TimeBar.Listener() {
                @Override public void onScrub(long position) {
                    handler.removeCallbacks(hideChrome);
                    showTime(position, player.getDuration());
                }

                @Override public void onSeek(long position) {
                    player.seekTo(position);
                    scheduleHide();
                }
            });
            addView(timeBar, new LayoutParams(-1, dp(20), Gravity.BOTTOM));

            gestures = new GestureDetector(activity, new GestureDetector.SimpleOnGestureListener() {
                @Override public boolean onDown(MotionEvent event) {
                    return true;
                }

                @Override public boolean onSingleTapConfirmed(MotionEvent event) {
                    if (mini) {
                        expand();
                    } else {
                        setChromeVisible(!chromeVisible);
                    }
                    return true;
                }

                @Override public boolean onDoubleTap(MotionEvent event) {
                    if (mini) {
                        expand();
                        return true;
                    }
                    float x = event.getX();
                    if (live || x > getWidth() / 3f && x < getWidth() * 2 / 3f) {
                        togglePlay();
                    } else if (x <= getWidth() / 3f) {
                        seekBy(-10000);
                        flashSeek(false);
                    } else {
                        seekBy(10000);
                        flashSeek(true);
                    }
                    return true;
                }
            });
            setChromeVisible(true);
        }

        void reset() {
            timeBar.setSegments(null);
            timeBar.setTimes(0, 0, 0);
            showTime(0, 0);
            updatePlaying();
            setChromeVisible(true);
        }

        void setMode(boolean mini, boolean wide) {
            this.wide = wide;
            fullscreenButton.setImageDrawable(PlayerIcons.of(wide ? PlayerIcons.FULLSCREEN_EXIT : PlayerIcons.FULLSCREEN, ON_VIDEO));
            fullscreenButton.setContentDescription(text(wide ? "Exit fullscreen" : "Fullscreen"));
            LayoutParams bar = (LayoutParams) timeBar.getLayoutParams();
            bar.bottomMargin = wide ? dp(28) : 0;
            bar.leftMargin = wide ? dp(16) : 0;
            bar.rightMargin = wide ? dp(16) : 0;
            timeBar.setLayoutParams(bar);
            LayoutParams row = (LayoutParams) bottomRow.getLayoutParams();
            row.bottomMargin = wide ? dp(44) : dp(12);
            bottomRow.setLayoutParams(row);
            bottomRow.setPadding(wide ? dp(28) : dp(12), 0, wide ? dp(16) : 0, 0);
            setChromeVisible(chromeVisible);
        }

        void setChromeVisible(boolean visible) {
            chromeVisible = visible && !mini;
            chrome.setVisibility(chromeVisible ? VISIBLE : INVISIBLE);
            timeBar.setExpanded(chromeVisible);
            timeBar.setAlignBottom(!chromeVisible);
            timeBar.setVisibility(mini || (wide && !chromeVisible) ? INVISIBLE : VISIBLE);
            if (chromeVisible) {
                scheduleHide();
            }
        }

        void updatePlaying() {
            int state = player.getPlaybackState();
            boolean ended = state == Player.STATE_ENDED;
            boolean playing = player.getPlayWhenReady() && state != Player.STATE_IDLE && !ended;
            int centre = errorBox.getVisibility() == View.VISIBLE ? View.INVISIBLE : View.VISIBLE;
            play.setVisibility(centre);
            previous.setVisibility(centre);
            next.setVisibility(centre);
            play.setImageDrawable(PlayerIcons.of(ended ? PlayerIcons.REPLAY : playing ? PlayerIcons.PAUSE : PlayerIcons.PLAY, ON_VIDEO));
            play.setContentDescription(text(playing ? "Pause" : "Play"));
            previous.setAlpha(index > 0 ? 1f : 0.4f);
            next.setAlpha(index + 1 < queue.size() || more ? 1f : 0.4f);
        }

        void updateTime(long position, long buffered, long duration) {
            timeBar.setEnabled(!live && duration > 0);
            timeBar.setTimes(position, buffered, live ? 0 : duration);
            if (!timeBar.isScrubbing()) {
                showTime(position, duration);
            }
        }

        private void showTime(long position, long duration) {
            if (live) {
                time.setText("● LIVE");
                time.setTextColor(ACCENT);
            } else {
                time.setTextColor(Color.WHITE);
                time.setText(TimeBar.format(position) + " / " + TimeBar.format(Math.max(0, duration)));
            }
        }

        private void flashSeek(boolean forward) {
            seekNote.setText(forward ? "10 s  »" : "«  10 s");
            LayoutParams params = (LayoutParams) seekNote.getLayoutParams();
            params.gravity = Gravity.CENTER_VERTICAL | (forward ? Gravity.END : Gravity.START);
            params.setMargins(dp(32), 0, dp(32), 0);
            seekNote.setLayoutParams(params);
            seekNote.setVisibility(VISIBLE);
            handler.removeCallbacks(hideSeekNote);
            handler.postDelayed(hideSeekNote, 700);
        }

        private final Runnable hideSeekNote = this::hideSeekNote;

        private void hideSeekNote() {
            seekNote.setVisibility(GONE);
        }

        @Override public boolean onInterceptTouchEvent(MotionEvent event) {
            // a downward drag anywhere on the player collapses it, even over a button
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    downX = event.getX();
                    downY = event.getY();
                    dragging = false;
                    return false;
                case MotionEvent.ACTION_MOVE:
                    return !mini && isDownSwipe(event);
                default:
                    return false;
            }
        }

        @Override public boolean onTouchEvent(MotionEvent event) {
            int action = event.getActionMasked();
            if (action == MotionEvent.ACTION_DOWN) {
                downX = event.getX();
                downY = event.getY();
                dragging = false;
            }
            if (!mini && action == MotionEvent.ACTION_MOVE && isDownSwipe(event)) {
                dragging = true;
            }
            if (dragging) {
                if (action == MotionEvent.ACTION_UP) {
                    dragging = false;
                    if (event.getY() - downY > dp(72)) {
                        if (wide) {
                            setFullscreen(false);
                        } else {
                            minimize();
                        }
                    }
                } else if (action == MotionEvent.ACTION_CANCEL) {
                    dragging = false;
                }
                return true;
            }
            gestures.onTouchEvent(event);
            return true;
        }

        private boolean isDownSwipe(MotionEvent event) {
            float dy = event.getY() - downY;
            return dy > dp(24) && dy > Math.abs(event.getX() - downX) * 1.5f;
        }
    }

    // ───────────── video surface ─────────────

    /** Fits its single child (the TextureView) to the video's aspect ratio, centred. With fill
     *  on, an upright video covers the frame and loses its edges, as Shorts do on YouTube. */
    private static final class VideoFrame extends FrameLayout {
        private float aspect = 16f / 9f;
        private boolean fill;

        void setFill(boolean fill) {
            this.fill = fill;
            requestLayout();
        }

        VideoFrame(Activity activity) {
            super(activity);
        }

        void setAspect(float aspect) {
            if (aspect > 0 && Math.abs(aspect - this.aspect) > 0.01f) {
                this.aspect = aspect;
                requestLayout();
            }
        }

        @Override protected void onMeasure(int widthSpec, int heightSpec) {
            int width = MeasureSpec.getSize(widthSpec);
            int height = MeasureSpec.getSize(heightSpec);
            setMeasuredDimension(width, height);
            int childWidth = width;
            int childHeight = Math.round(width / aspect);
            if (childHeight > height) {
                childHeight = height;
                childWidth = Math.round(height * aspect);
            }
            if (fill && aspect < 0.8f && width > 0 && width < height * 0.8f) {
                childWidth = Math.max(width, Math.round(height * aspect));
                childHeight = Math.round(childWidth / aspect);
            }
            for (int i = 0; i < getChildCount(); i++) {
                getChildAt(i).measure(MeasureSpec.makeMeasureSpec(childWidth, MeasureSpec.EXACTLY),
                    MeasureSpec.makeMeasureSpec(childHeight, MeasureSpec.EXACTLY));
            }
        }

        @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
            for (int i = 0; i < getChildCount(); i++) {
                View child = getChildAt(i);
                int x = (right - left - child.getMeasuredWidth()) / 2;
                int y = (bottom - top - child.getMeasuredHeight()) / 2;
                child.layout(x, y, x + child.getMeasuredWidth(), y + child.getMeasuredHeight());
            }
        }
    }

    /** Vertical swipes move through the Shorts queue; other touches pass through. */
    private static final class ShortsGesture extends FrameLayout {
        interface Move {
            void by(int direction);
        }

        private final Move move;
        private final int threshold;
        private float x;
        private float y;
        private boolean swiping;
        private boolean multiTouch;

        ShortsGesture(Activity activity, Move move) {
            super(activity);
            this.move = move;
            threshold = Math.round(activity.getResources().getDisplayMetrics().density * 42);
        }

        @Override public boolean dispatchTouchEvent(MotionEvent event) {
            int action = event.getActionMasked();
            if (action == MotionEvent.ACTION_DOWN) {
                x = event.getX();
                y = event.getY();
                swiping = false;
                multiTouch = false;
            } else if (action == MotionEvent.ACTION_POINTER_DOWN) {
                multiTouch = true;
            }
            if (!swiping && !multiTouch && action == MotionEvent.ACTION_MOVE && isSwipe(event)) {
                MotionEvent cancelled = MotionEvent.obtain(event);
                cancelled.setAction(MotionEvent.ACTION_CANCEL);
                super.dispatchTouchEvent(cancelled);
                cancelled.recycle();
                swiping = true;
            }
            if (!swiping) {
                return super.dispatchTouchEvent(event);
            }
            if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
                swiping = false;
                if (!multiTouch && action == MotionEvent.ACTION_UP && isSwipe(event)) {
                    move.by(event.getY() < y ? 1 : -1);
                }
            }
            return true;
        }

        private boolean isSwipe(MotionEvent event) {
            float dy = Math.abs(event.getY() - y);
            return dy > threshold && dy > Math.abs(event.getX() - x) * 1.3f;
        }
    }

    /** Predictive back (API 33+) reaches the dialog through this callback. */
    private static final class BackCallbacks {
        static void register(Dialog dialog, PlayerSession session) {
            dialog.getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT, () -> {
                    if (!session.back()) {
                        session.dismiss();
                    }
                });
        }
    }
}
