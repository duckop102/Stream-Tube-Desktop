package io.github.shiroksh.streamtube;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.view.MotionEvent;
import android.view.View;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONObject;

/** The red YouTube-style seek bar with SponsorBlock segment marks. */
final class TimeBar extends View {
    interface Listener {
        void onScrub(long position);

        void onSeek(long position);
    }

    private final Paint track = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint buffer = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint played = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint skip = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint mark = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final float density;
    private Listener listener;
    private long duration;
    private long position;
    private long buffered;
    private JSONArray segments;
    private boolean scrubbing;
    private boolean expanded = true;
    private boolean alignBottom;

    TimeBar(Context context) {
        super(context);
        density = context.getResources().getDisplayMetrics().density;
        track.setColor(0x4DFFFFFF);
        buffer.setColor(0x80FFFFFF);
        played.setColor(PlayerSession.ACCENT);
        skip.setColor(0xFF00D400);
        mark.setColor(0xFFFFD600);
    }

    void setListener(Listener listener) {
        this.listener = listener;
    }

    /** Expanded shows the thumb and takes touches; collapsed is a thin progress line. */
    void setExpanded(boolean expanded) {
        if (this.expanded != expanded) {
            this.expanded = expanded;
            invalidate();
        }
    }

    /** Draws the line on the bottom edge (over the video) instead of centred. */
    void setAlignBottom(boolean alignBottom) {
        this.alignBottom = alignBottom;
        invalidate();
    }

    void setSegments(JSONArray segments) {
        this.segments = segments;
        invalidate();
    }

    void setTimes(long position, long buffered, long duration) {
        this.duration = Math.max(0, duration);
        this.buffered = buffered;
        if (!scrubbing) {
            this.position = position;
        }
        invalidate();
    }

    boolean isScrubbing() {
        return scrubbing;
    }

    @Override protected void onDraw(Canvas canvas) {
        float width = getWidth();
        float thickness = (scrubbing ? 5 : expanded ? 3 : 2) * density;
        float center = alignBottom ? getHeight() - thickness / 2 : getHeight() / 2f;
        float top = center - thickness / 2;
        float bottom = center + thickness / 2;
        canvas.drawRect(0, top, width, bottom, track);
        if (duration <= 0) {
            return;
        }
        canvas.drawRect(0, top, width * fraction(buffered), bottom, buffer);
        if (segments != null) {
            for (int i = 0; i < segments.length(); i++) {
                JSONObject segment = segments.optJSONObject(i);
                if (segment == null) {
                    continue;
                }
                double start = segment.optDouble("start", 0) * 1000;
                double end = segment.optDouble("end", 0) * 1000;
                if (start >= duration || end <= start) {
                    continue;
                }
                float left = (float) (width * start / duration);
                float right = Math.max(left + density, (float) (width * Math.min(end, duration) / duration));
                canvas.drawRect(left, top, right, bottom, segment.optBoolean("auto_skip") ? skip : mark);
            }
        }
        float x = width * fraction(position);
        canvas.drawRect(0, top, x, bottom, played);
        if (expanded || scrubbing) {
            float radius = (scrubbing ? 8 : 6) * density;
            canvas.drawCircle(Math.max(radius, Math.min(width - radius, x)), center, radius, played);
        }
    }

    private float fraction(long value) {
        return duration <= 0 ? 0 : Math.max(0, Math.min(1, value / (float) duration));
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (!expanded || duration <= 0 || !isEnabled()) {
            return false;
        }
        long at = (long) (duration * Math.max(0, Math.min(1, event.getX() / Math.max(1, getWidth()))));
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                scrubbing = true;
                getParent().requestDisallowInterceptTouchEvent(true);
                position = at;
                if (listener != null) {
                    listener.onScrub(at);
                }
                invalidate();
                return true;
            case MotionEvent.ACTION_MOVE:
                position = at;
                if (listener != null) {
                    listener.onScrub(at);
                }
                invalidate();
                return true;
            case MotionEvent.ACTION_UP:
                scrubbing = false;
                position = at;
                if (listener != null) {
                    listener.onSeek(at);
                }
                invalidate();
                return true;
            case MotionEvent.ACTION_CANCEL:
                scrubbing = false;
                invalidate();
                return true;
            default:
                return true;
        }
    }

    static String format(long millis) {
        long elapsed = Math.max(0, millis / 1000);
        long seconds = elapsed % 60;
        if (elapsed < 3600) {
            return String.format(Locale.ROOT, "%d:%02d", elapsed / 60, seconds);
        }
        return String.format(Locale.ROOT, "%d:%02d:%02d", elapsed / 3600, elapsed / 60 % 60, seconds);
    }
}
