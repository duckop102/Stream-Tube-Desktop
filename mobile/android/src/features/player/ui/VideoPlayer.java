package io.github.shiroksh.streamtube;

import android.app.Activity;
import android.widget.Toast;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Entry points Rust calls (static, on any thread) and the one native callback.
 * Every call names its playback session; calls for an earlier session are ignored.
 */
public final class VideoPlayer {
    static final Pattern VIDEO_ID = Pattern.compile("[A-Za-z0-9_-]{11}");
    static final Pattern CHANNEL_ID = Pattern.compile("UC[A-Za-z0-9_-]{22}");

    // Event kinds, mirrored in src/features/player/mod.rs (`player::event`).
    static final int WATCHED = 0;
    static final int SAVE = 1;
    static final int CHANNEL = 2;
    static final int CLOSED = 3;
    static final int MORE = 4;
    static final int STREAM = 5;
    static final int COMMENTS = 7;
    static final int RELATED = 8;
    static final int SUBSCRIBE = 9;
    static final int FOLLOWING = 10;

    static PlayerSession active;

    private VideoPlayer() {}

    public static void open(Activity activity, int session, String items, int first,
                            String language, String messages, boolean shorts, boolean more) {
        activity.runOnUiThread(() -> {
            if (activity.isFinishing() || activity.isDestroyed()) {
                event(session, CLOSED, "");
                return;
            }
            JSONObject labels = new JSONObject();
            try {
                labels = new JSONObject(messages);
                List<Track> queue = decode(items);
                if (first < 0 || first >= queue.size()) {
                    event(session, CLOSED, "");
                    return;
                }
                if (active != null) {
                    active.dismiss();
                }
                active = new PlayerSession(activity, session, queue, first, language, labels, shorts, more);
                active.show();
            } catch (RuntimeException | JSONException failure) {
                if (active != null && active.session == session) {
                    active.dismiss();
                }
                event(session, CLOSED, "");
                Toast.makeText(activity, labels.optString("Player unavailable", "Player unavailable"),
                    Toast.LENGTH_SHORT).show();
            }
        });
    }

    public static void append(Activity activity, int session, String items, boolean more, boolean failed) {
        run(activity, session, player -> {
            try {
                player.append(decode(items), more, failed);
            } catch (JSONException failure) {
                player.append(new ArrayList<>(), more, true);
            }
        });
    }

    public static void updateSaved(Activity activity, int session, String id, boolean saved) {
        run(activity, session, player -> player.updateSaved(id, saved));
    }

    public static void setExtras(Activity activity, int session, String id, String packet) {
        withPacket(activity, session, id, packet, PlayerSession::extras);
    }

    public static void setStream(Activity activity, int session, String id, String packet) {
        withPacket(activity, session, id, packet, PlayerSession::stream);
    }

    public static void setDetails(Activity activity, int session, String id, String packet) {
        withPacket(activity, session, id, packet, PlayerSession::details);
    }

    public static void setComments(Activity activity, int session, String id, String packet) {
        withPacket(activity, session, id, packet, PlayerSession::comments);
    }

    public static void setFollowing(Activity activity, int session, String channel, boolean following) {
        run(activity, session, player -> player.following(channel, following));
    }

    private interface PacketHandler {
        void handle(PlayerSession player, JSONObject packet);
    }

    private interface SessionTask {
        void run(PlayerSession player);
    }

    private static void run(Activity activity, int session, SessionTask task) {
        activity.runOnUiThread(() -> {
            PlayerSession player = active;
            if (player != null && player.session == session) {
                task.run(player);
            }
        });
    }

    private static void withPacket(Activity activity, int session, String id, String packet,
                                   PacketHandler handler) {
        run(activity, session, player -> {
            if (!player.track().id.equals(id)) {
                return;
            }
            try {
                handler.handle(player, new JSONObject(packet));
            } catch (JSONException invalid) {
                // Optional metadata: the video plays without it.
            }
        });
    }

    static List<Track> decode(String items) throws JSONException {
        JSONArray decoded = new JSONArray(items);
        List<Track> tracks = new ArrayList<>(decoded.length());
        for (int i = 0; i < decoded.length(); i++) {
            Track track = new Track(decoded.getJSONObject(i));
            if (!VIDEO_ID.matcher(track.id).matches()) {
                throw new JSONException("Invalid video identifier");
            }
            tracks.add(track);
        }
        return tracks;
    }

    static final class Track {
        final String id;
        final String title;
        final String channel;
        final boolean channelLink;
        final String avatar;
        boolean saved;

        Track(JSONObject item) throws JSONException {
            id = item.getString("id");
            title = item.getString("title");
            channel = item.getString("channel");
            channelLink = item.getBoolean("channel_link");
            avatar = item.optString("avatar", "");
            saved = item.getBoolean("saved");
        }

        /** A related video from the watch page details. */
        Track(String id, String title, String channel, boolean channelLink) {
            this.id = id;
            this.title = title;
            this.channel = channel;
            this.channelLink = channelLink;
            this.avatar = "";
        }
    }

    static native void event(int session, int kind, String id);
}
