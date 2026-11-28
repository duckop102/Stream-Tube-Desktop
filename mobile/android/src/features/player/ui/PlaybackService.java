package io.github.shiroksh.streamtube;

import android.content.Context;
import android.content.Intent;
import androidx.annotation.Nullable;
import androidx.annotation.OptIn;
import androidx.media3.common.util.UnstableApi;
import androidx.media3.session.MediaSession;
import androidx.media3.session.MediaSessionService;

/**
 * Keeps a video playing while Stream Tube is in the background or the screen is off,
 * with its media notification, lock screen and headset controls. The player and its
 * session belong to the open PlayerSession; this service only hosts that session.
 */
@OptIn(markerClass = UnstableApi.class)
public final class PlaybackService extends MediaSessionService {
    private static MediaSession current;
    private static PlaybackService running;

    /** Hosts the session of the player that just opened. */
    static void attach(Context context, MediaSession session) {
        current = session;
        if (running != null) {
            running.addSession(session);
            return;
        }
        try {
            context.startService(new Intent(context, PlaybackService.class));
        } catch (RuntimeException refused) {
            // the video still plays while the app is open
        }
    }

    /** Lets go of a player that closed; the service ends with the last one. */
    static void detach(MediaSession session) {
        if (current == session) {
            current = null;
        }
        PlaybackService service = running;
        if (service == null) {
            return;
        }
        if (service.isSessionAdded(session)) {
            service.removeSession(session);
        }
        if (current == null) {
            service.stopSelf();
        }
    }

    @Override public void onCreate() {
        super.onCreate();
        running = this;
        if (current != null) {
            addSession(current);
        }
    }

    @Override public int onStartCommand(@Nullable Intent intent, int flags, int startId) {
        super.onStartCommand(intent, flags, startId);
        if (current == null) {
            stopSelf();
        }
        // nothing to resume after the system ends the app
        return START_NOT_STICKY;
    }

    @Nullable
    @Override public MediaSession onGetSession(MediaSession.ControllerInfo controller) {
        return current;
    }

    /** Swiping the app away stops the video, as in the YouTube app. */
    @Override public void onTaskRemoved(@Nullable Intent rootIntent) {
        MediaSession session = current;
        if (session != null) {
            session.getPlayer().pause();
        }
        stopSelf();
    }

    @Override public void onDestroy() {
        if (running == this) {
            running = null;
        }
        super.onDestroy();
    }
}
