package io.github.shiroksh.streamtube;

import android.app.Activity;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.icu.text.CompactDecimalFormat;
import android.text.TextUtils;
import android.text.method.LinkMovementMethod;
import android.text.util.Linkify;
import android.view.Gravity;
import android.view.View;
import android.view.ViewOutlineProvider;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONObject;

/** The scrolling part of the watch page under the video, and its comments sheet. */
final class WatchPage {
    private static final int TEXT = PlayerSession.TEXT;
    private static final int SECONDARY = PlayerSession.SECONDARY;
    private static final int SURFACE = PlayerSession.SURFACE;

    private final PlayerSession s;
    private final Activity activity;
    private final Typeface medium = Typeface.create("sans-serif-medium", Typeface.NORMAL);
    private final FrameLayout container;
    private final ScrollView scroll;
    private final TextView title;
    private final TextView meta;
    private final LinearLayout channelRow;
    private final ImageView avatar;
    private final TextView channelName;
    private final TextView subscribers;
    private final TextView subscribe;
    private final TextView likes;
    private final TextView dislikes;
    private final TextView save;
    private final LinearLayout descriptionCard;
    private final TextView descriptionMeta;
    private final TextView description;
    private final TextView descriptionToggle;
    private final LinearLayout commentsCard;
    private final TextView commentsCount;
    private final ImageView commentAvatar;
    private final TextView commentText;
    private final LinearLayout related;
    private final LinearLayout sheet;
    private final TextView sheetCount;
    private final ScrollView sheetScroll;
    private final LinearLayout sheetList;
    private final ProgressBar sheetSpinner;
    private final TextView sheetFooter;

    private String trackId = "";
    private String channelId = "";
    private String channelTitle = "";
    private String detailsTitle = "";
    private String alternateTitle = "";
    private String views = "";
    private String date = "";
    private boolean expanded;
    private String commentsFirst;
    private String commentsNext;
    private String commentsPending;
    private boolean commentsFailed;
    private int commentsShown;

    WatchPage(PlayerSession session) {
        s = session;
        activity = session.activity;
        container = new FrameLayout(activity);
        scroll = new ScrollView(activity);
        scroll.setFillViewport(true);
        container.addView(scroll, new FrameLayout.LayoutParams(-1, -1));
        LinearLayout content = new LinearLayout(activity);
        content.setOrientation(LinearLayout.VERTICAL);
        content.setPadding(0, s.dp(12), 0, s.dp(24));
        scroll.addView(content, new ScrollView.LayoutParams(-1, -2));

        title = s.label("", 18, TEXT);
        title.setTypeface(medium);
        title.setMaxLines(2);
        title.setEllipsize(TextUtils.TruncateAt.END);
        title.setPadding(s.dp(12), 0, s.dp(12), 0);
        title.setOnClickListener(view -> setExpanded(!expanded));
        content.addView(title, new LinearLayout.LayoutParams(-1, -2));

        meta = s.label("", 12, SECONDARY);
        meta.setPadding(s.dp(12), s.dp(6), s.dp(12), 0);
        meta.setOnClickListener(view -> setExpanded(!expanded));
        content.addView(meta, new LinearLayout.LayoutParams(-1, -2));

        // channel row
        channelRow = new LinearLayout(activity);
        channelRow.setGravity(Gravity.CENTER_VERTICAL);
        channelRow.setPadding(s.dp(12), 0, s.dp(12), 0);
        LinearLayout owner = new LinearLayout(activity);
        owner.setGravity(Gravity.CENTER_VERTICAL);
        owner.setBackground(s.ripple());
        owner.setOnClickListener(view -> s.openChannel(channelId, channelTitle));
        avatar = circle(36);
        owner.addView(avatar, new LinearLayout.LayoutParams(s.dp(36), s.dp(36)));
        LinearLayout names = new LinearLayout(activity);
        names.setOrientation(LinearLayout.VERTICAL);
        names.setPadding(s.dp(12), 0, s.dp(8), 0);
        channelName = s.label("", 15, TEXT);
        channelName.setTypeface(medium);
        channelName.setSingleLine(true);
        channelName.setEllipsize(TextUtils.TruncateAt.END);
        subscribers = s.label("", 12, SECONDARY);
        subscribers.setSingleLine(true);
        names.addView(channelName);
        names.addView(subscribers);
        owner.addView(names, new LinearLayout.LayoutParams(0, -2, 1));
        channelRow.addView(owner, new LinearLayout.LayoutParams(0, s.dp(56), 1));
        subscribe = s.label(s.text("Subscribe"), 14, Color.BLACK);
        subscribe.setTypeface(medium);
        subscribe.setGravity(Gravity.CENTER);
        subscribe.setPadding(s.dp(16), 0, s.dp(16), 0);
        subscribe.setOnClickListener(view -> {
            if (VideoPlayer.CHANNEL_ID.matcher(channelId).matches()) {
                VideoPlayer.event(s.session, VideoPlayer.SUBSCRIBE, channelId + "\n" + channelTitle);
            }
        });
        channelRow.addView(subscribe, new LinearLayout.LayoutParams(-2, s.dp(36)));
        LinearLayout.LayoutParams channelParams = new LinearLayout.LayoutParams(-1, -2);
        channelParams.topMargin = s.dp(6);
        content.addView(channelRow, channelParams);

        // pills
        HorizontalScrollView pillScroll = new HorizontalScrollView(activity);
        pillScroll.setHorizontalScrollBarEnabled(false);
        LinearLayout pills = new LinearLayout(activity);
        pills.setPadding(s.dp(12), 0, s.dp(12), 0);
        LinearLayout rating = new LinearLayout(activity);
        rating.setGravity(Gravity.CENTER_VERTICAL);
        rating.setBackground(s.rounded(SURFACE, s.dp(18)));
        likes = ratingHalf(PlayerIcons.LIKE);
        rating.addView(likes, new LinearLayout.LayoutParams(-2, -1));
        View divider = new View(activity);
        divider.setBackgroundColor(0x33FFFFFF);
        rating.addView(divider, new LinearLayout.LayoutParams(s.dp(1), s.dp(22)));
        dislikes = ratingHalf(PlayerIcons.DISLIKE);
        rating.addView(dislikes, new LinearLayout.LayoutParams(-2, -1));
        pills.addView(rating, new LinearLayout.LayoutParams(-2, s.dp(36)));
        TextView share = s.pill(s.text("Share"), PlayerIcons.SHARE, 20);
        share.setOnClickListener(view -> s.share());
        pills.addView(share, pillParams());
        save = s.pill(s.text("Save"), PlayerIcons.SAVE, 20);
        save.setOnClickListener(view -> s.save());
        pills.addView(save, pillParams());
        pillScroll.addView(pills);
        LinearLayout.LayoutParams pillScrollParams = new LinearLayout.LayoutParams(-1, -2);
        pillScrollParams.topMargin = s.dp(4);
        content.addView(pillScroll, pillScrollParams);

        // description
        descriptionCard = card();
        descriptionCard.setOnClickListener(view -> {
            if (!expanded) {
                setExpanded(true);
            }
        });
        descriptionMeta = s.label("", 13, TEXT);
        descriptionMeta.setTypeface(medium);
        descriptionCard.addView(descriptionMeta);
        description = s.label("", 13, TEXT);
        description.setLineSpacing(0, 1.15f);
        LinearLayout.LayoutParams descriptionParams = new LinearLayout.LayoutParams(-1, -2);
        descriptionParams.topMargin = s.dp(4);
        descriptionCard.addView(description, descriptionParams);
        descriptionToggle = s.label(s.text("...more"), 13, TEXT);
        descriptionToggle.setTypeface(medium);
        descriptionToggle.setPadding(0, s.dp(6), 0, 0);
        descriptionToggle.setOnClickListener(view -> setExpanded(!expanded));
        descriptionCard.addView(descriptionToggle);
        content.addView(descriptionCard, cardParams());

        // comments teaser
        commentsCard = card();
        commentsCard.setOnClickListener(view -> openComments());
        LinearLayout heading = new LinearLayout(activity);
        heading.setGravity(Gravity.CENTER_VERTICAL);
        TextView commentsTitle = s.label(s.text("Comments"), 14, TEXT);
        commentsTitle.setTypeface(medium);
        heading.addView(commentsTitle);
        commentsCount = s.label("", 13, SECONDARY);
        commentsCount.setPadding(s.dp(8), 0, 0, 0);
        heading.addView(commentsCount);
        commentsCard.addView(heading);
        LinearLayout first = new LinearLayout(activity);
        first.setGravity(Gravity.CENTER_VERTICAL);
        commentAvatar = circle(24);
        first.addView(commentAvatar, new LinearLayout.LayoutParams(s.dp(24), s.dp(24)));
        commentText = s.label("", 13, TEXT);
        commentText.setMaxLines(2);
        commentText.setEllipsize(TextUtils.TruncateAt.END);
        commentText.setPadding(s.dp(10), 0, 0, 0);
        first.addView(commentText, new LinearLayout.LayoutParams(0, -2, 1));
        LinearLayout.LayoutParams firstParams = new LinearLayout.LayoutParams(-1, -2);
        firstParams.topMargin = s.dp(8);
        commentsCard.addView(first, firstParams);
        content.addView(commentsCard, cardParams());

        related = new LinearLayout(activity);
        related.setOrientation(LinearLayout.VERTICAL);
        LinearLayout.LayoutParams relatedParams = new LinearLayout.LayoutParams(-1, -2);
        relatedParams.topMargin = s.dp(8);
        content.addView(related, relatedParams);

        // comments sheet over the details
        sheet = new LinearLayout(activity);
        sheet.setOrientation(LinearLayout.VERTICAL);
        sheet.setClickable(true);
        GradientDrawable sheetBackground = new GradientDrawable();
        sheetBackground.setColor(PlayerSession.MINI_SURFACE);
        float radius = s.dp(14);
        sheetBackground.setCornerRadii(new float[]{radius, radius, radius, radius, 0, 0, 0, 0});
        sheet.setBackground(sheetBackground);
        LinearLayout sheetHeader = new LinearLayout(activity);
        sheetHeader.setGravity(Gravity.CENTER_VERTICAL);
        sheetHeader.setPadding(s.dp(16), 0, s.dp(4), 0);
        TextView sheetTitle = s.label(s.text("Comments"), 18, TEXT);
        sheetTitle.setTypeface(medium);
        sheetHeader.addView(sheetTitle);
        sheetCount = s.label("", 14, SECONDARY);
        sheetCount.setPadding(s.dp(8), 0, 0, 0);
        sheetHeader.addView(sheetCount, new LinearLayout.LayoutParams(0, -2, 1));
        ImageView close = s.iconButton(PlayerIcons.CLOSE, 24, s.text("Close"));
        close.setOnClickListener(view -> closeComments());
        sheetHeader.addView(close, new LinearLayout.LayoutParams(s.dp(48), s.dp(48)));
        sheet.addView(sheetHeader, new LinearLayout.LayoutParams(-1, s.dp(56)));
        View rule = new View(activity);
        rule.setBackgroundColor(0x1FFFFFFF);
        sheet.addView(rule, new LinearLayout.LayoutParams(-1, s.dp(1)));
        sheetScroll = new ScrollView(activity);
        LinearLayout sheetContent = new LinearLayout(activity);
        sheetContent.setOrientation(LinearLayout.VERTICAL);
        sheetList = new LinearLayout(activity);
        sheetList.setOrientation(LinearLayout.VERTICAL);
        sheetContent.addView(sheetList);
        sheetSpinner = new ProgressBar(activity);
        sheetSpinner.setIndeterminateTintList(ColorStateList.valueOf(SECONDARY));
        LinearLayout.LayoutParams spinnerParams = new LinearLayout.LayoutParams(s.dp(32), s.dp(32));
        spinnerParams.gravity = Gravity.CENTER_HORIZONTAL;
        spinnerParams.setMargins(0, s.dp(16), 0, s.dp(16));
        sheetContent.addView(sheetSpinner, spinnerParams);
        sheetFooter = s.label("", 14, SECONDARY);
        sheetFooter.setGravity(Gravity.CENTER);
        sheetFooter.setPadding(s.dp(16), s.dp(16), s.dp(16), s.dp(24));
        sheetFooter.setOnClickListener(view -> {
            if (commentsFailed && commentsNext != null) {
                requestComments(commentsNext);
            }
        });
        sheetContent.addView(sheetFooter, new LinearLayout.LayoutParams(-1, -2));
        sheetScroll.addView(sheetContent, new ScrollView.LayoutParams(-1, -2));
        sheetScroll.setOnScrollChangeListener((view, x, y, oldX, oldY) -> maybeLoadMore());
        sheet.addView(sheetScroll, new LinearLayout.LayoutParams(-1, 0, 1));
        sheet.setVisibility(View.GONE);
        container.addView(sheet, new FrameLayout.LayoutParams(-1, -1));
    }

    View view() {
        return container;
    }

    void bind(VideoPlayer.Track track, boolean hasNext) {
        trackId = track.id;
        channelId = "";
        channelTitle = track.channel;
        detailsTitle = track.title;
        alternateTitle = "";
        views = "";
        date = "";
        commentsFirst = null;
        commentsNext = null;
        commentsPending = null;
        commentsFailed = false;
        commentsShown = 0;
        title.setText(track.title);
        meta.setText("");
        meta.setVisibility(View.GONE);
        channelName.setText(track.channel);
        subscribers.setText("");
        subscribers.setVisibility(View.GONE);
        RemoteImages.load(avatar, "", 0);
        channelRow.setVisibility(track.channel.isEmpty() ? View.GONE : View.VISIBLE);
        setSubscribed(false);
        subscribe.setEnabled(false);
        subscribe.setAlpha(0.5f);
        likes.setText("");
        dislikes.setText("");
        descriptionCard.setVisibility(View.GONE);
        description.setText("");
        commentsCard.setVisibility(View.GONE);
        commentText.setText("");
        RemoteImages.load(commentAvatar, "", 0);
        related.removeAllViews();
        sheetList.removeAllViews();
        sheetCount.setText("");
        sheet.setVisibility(View.GONE);
        setExpanded(false);
        scroll.scrollTo(0, 0);
    }

    void queueState(boolean hasNext) {
        // the controls show whether a next video exists; nothing to draw here yet
    }

    boolean back() {
        if (sheet.getVisibility() == View.VISIBLE) {
            closeComments();
            return true;
        }
        return false;
    }

    // ───────────── packets ─────────────

    void details(JSONObject packet) {
        String value = packet.optString("title", "");
        if (!value.isEmpty()) {
            detailsTitle = value;
        }
        title.setText(alternateTitle.isEmpty() ? detailsTitle : alternateTitle);
        views = packet.optString("views", "");
        date = packet.optString("date", "");
        String line = join("  ", views, date);
        meta.setText(line);
        meta.setVisibility(line.isEmpty() ? View.GONE : View.VISIBLE);

        JSONObject channel = packet.optJSONObject("channel");
        if (channel != null) {
            String id = channel.optString("id", "");
            String name = channel.optString("name", "");
            if (!name.isEmpty()) {
                channelTitle = name;
                channelName.setText(name);
                channelRow.setVisibility(View.VISIBLE);
            }
            String subs = channel.optString("subscribers", "");
            subscribers.setText(subs);
            subscribers.setVisibility(subs.isEmpty() ? View.GONE : View.VISIBLE);
            RemoteImages.load(avatar, channel.optString("avatar", ""), s.dp(36));
            if (VideoPlayer.CHANNEL_ID.matcher(id).matches()) {
                channelId = id;
                VideoPlayer.event(s.session, VideoPlayer.FOLLOWING, channelId + "\n" + channelTitle);
            }
        }

        likes.setText(packet.optString("likes", ""));

        String text = packet.optString("description", "");
        descriptionMeta.setText(join("  ", views, date));
        description.setText(text);
        description.setVisibility(text.isEmpty() ? View.GONE : View.VISIBLE);
        descriptionCard.setVisibility(text.isEmpty() && views.isEmpty() ? View.GONE : View.VISIBLE);
        setExpanded(expanded);

        String count = packet.optString("comments_count", "");
        String token = packet.optString("comments_token", "");
        commentsCount.setText(count);
        sheetCount.setText(count);
        if (!token.isEmpty() && !packet.isNull("comments_token")) {
            commentsFirst = token;
            commentsCard.setVisibility(View.VISIBLE);
            commentText.setText("…");
            requestComments(token);
        }

        related.removeAllViews();
        JSONArray items = packet.optJSONArray("related");
        if (items != null) {
            for (int i = 0; i < items.length(); i++) {
                JSONObject item = items.optJSONObject(i);
                if (item != null && VideoPlayer.VIDEO_ID.matcher(item.optString("id", "")).matches()) {
                    related.addView(relatedRow(item));
                }
            }
        }
    }

    void extras(JSONObject packet) {
        if (!packet.isNull("title") && !packet.optString("title", "").isEmpty()) {
            alternateTitle = packet.optString("title");
            title.setText(alternateTitle);
        }
        if (!packet.isNull("dislikes")) {
            dislikes.setText(compact(packet.optLong("dislikes")));
        }
    }

    void saved(boolean saved) {
        save.setText(saved ? s.text("Saved") : s.text("Save"));
        save.setCompoundDrawablesRelative(s.icon(saved ? PlayerIcons.SAVED : PlayerIcons.SAVE, 20), null, null, null);
    }

    void following(String channel, boolean following) {
        if (!channel.equals(channelId)) {
            return;
        }
        subscribe.setEnabled(true);
        subscribe.setAlpha(1f);
        setSubscribed(following);
    }

    void comments(JSONObject packet) {
        String from = packet.optString("from", "");
        if (commentsPending == null || !commentsPending.equals(from)) {
            return;
        }
        commentsPending = null;
        sheetSpinner.setVisibility(View.GONE);
        if (packet.optBoolean("error", true)) {
            commentsFailed = true;
            commentsNext = from;
            sheetFooter.setText(s.text("Could not load comments. Tap to retry"));
            sheetFooter.setVisibility(View.VISIBLE);
            if (from.equals(commentsFirst) && commentsShown == 0) {
                commentText.setText(s.text("Could not load comments"));
            }
            return;
        }
        commentsFailed = false;
        JSONObject page = packet.optJSONObject("page");
        if (page == null) {
            return;
        }
        String count = page.optString("count", "");
        if (!count.isEmpty() && commentsCount.getText().length() == 0) {
            commentsCount.setText(count);
            sheetCount.setText(count);
        }
        JSONArray list = page.optJSONArray("comments");
        if (list != null) {
            for (int i = 0; i < list.length(); i++) {
                JSONObject comment = list.optJSONObject(i);
                if (comment == null) {
                    continue;
                }
                if (commentsShown == 0) {
                    commentText.setText(comment.optString("text", ""));
                    RemoteImages.load(commentAvatar, comment.optString("avatar", ""), s.dp(24));
                }
                sheetList.addView(commentRow(comment));
                commentsShown++;
            }
        }
        commentsNext = page.isNull("next") ? null : page.optString("next", null);
        if (commentsShown == 0 && commentsNext == null) {
            commentText.setText(s.text("No comments yet"));
        }
        sheetFooter.setText(commentsShown == 0 ? s.text("No comments yet") : "");
        sheetFooter.setVisibility(commentsNext == null && commentsShown == 0 ? View.VISIBLE : View.GONE);
        sheetScroll.post(this::maybeLoadMore);
    }

    // ───────────── behaviour ─────────────

    private void setExpanded(boolean value) {
        expanded = value;
        title.setMaxLines(value ? Integer.MAX_VALUE : 2);
        description.setMaxLines(value ? Integer.MAX_VALUE : 3);
        description.setEllipsize(value ? null : TextUtils.TruncateAt.END);
        if (value) {
            Linkify.addLinks(description, Linkify.WEB_URLS);
            description.setLinkTextColor(0xFF3EA6FF);
            description.setMovementMethod(LinkMovementMethod.getInstance());
        } else {
            description.setText(description.getText().toString());
            description.setMovementMethod(null);
            description.setFocusable(false);
            description.setClickable(false);
            description.setLongClickable(false);
        }
        descriptionToggle.setText(value ? s.text("Show less") : s.text("...more"));
    }

    private void requestComments(String token) {
        commentsPending = token;
        sheetSpinner.setVisibility(View.VISIBLE);
        sheetFooter.setVisibility(View.GONE);
        VideoPlayer.event(s.session, VideoPlayer.COMMENTS, trackId + "\n" + token);
    }

    private void maybeLoadMore() {
        if (sheet.getVisibility() != View.VISIBLE || commentsNext == null || commentsPending != null
            || commentsFailed) {
            return;
        }
        int remaining = sheetList.getBottom() - (sheetScroll.getScrollY() + sheetScroll.getHeight());
        if (remaining < s.dp(800)) {
            requestComments(commentsNext);
        }
    }

    private void openComments() {
        if (commentsFirst == null) {
            return;
        }
        sheet.setVisibility(View.VISIBLE);
        sheet.setTranslationY(container.getHeight());
        sheet.animate().translationY(0).setDuration(220).start();
        if (commentsShown == 0 && commentsPending == null) {
            commentsFailed = false;
            requestComments(commentsNext != null ? commentsNext : commentsFirst);
        } else {
            sheetScroll.post(this::maybeLoadMore);
        }
    }

    private void closeComments() {
        sheet.animate().cancel();
        sheet.setVisibility(View.GONE);
    }

    private void setSubscribed(boolean following) {
        subscribe.setText(following ? s.text("Subscribed") : s.text("Subscribe"));
        subscribe.setTextColor(following ? TEXT : Color.BLACK);
        subscribe.setBackground(new RippleDrawable(ColorStateList.valueOf(0x33808080),
            s.rounded(following ? SURFACE : TEXT, s.dp(18)), null));
    }

    // ───────────── rows ─────────────

    private View relatedRow(JSONObject item) {
        LinearLayout row = new LinearLayout(activity);
        row.setOrientation(LinearLayout.VERTICAL);
        row.setBackground(s.ripple());
        row.setOnClickListener(view -> s.playRelated(item));
        FrameLayout frame = new FrameLayout(activity);
        ImageView thumbnail = new RatioImage(activity);
        thumbnail.setScaleType(ImageView.ScaleType.CENTER_CROP);
        thumbnail.setBackgroundColor(SURFACE);
        frame.addView(thumbnail, new FrameLayout.LayoutParams(-1, -2));
        String id = item.optString("id");
        RemoteImages.load(thumbnail, RemoteImages.thumbnail(id),
            activity.getResources().getDisplayMetrics().widthPixels);
        String duration = item.optString("duration", "");
        if (!duration.isEmpty()) {
            TextView badge = s.label(duration, 12, Color.WHITE);
            badge.setTypeface(medium);
            badge.setPadding(s.dp(4), s.dp(1), s.dp(4), s.dp(1));
            badge.setBackground(s.rounded("LIVE".equalsIgnoreCase(duration) ? 0xFFCC0000 : 0xCC000000, s.dp(4)));
            FrameLayout.LayoutParams badgeParams = new FrameLayout.LayoutParams(-2, -2, Gravity.BOTTOM | Gravity.END);
            badgeParams.setMargins(0, 0, s.dp(8), s.dp(8));
            frame.addView(badge, badgeParams);
        }
        row.addView(frame, new LinearLayout.LayoutParams(-1, -2));
        LinearLayout text = new LinearLayout(activity);
        text.setOrientation(LinearLayout.VERTICAL);
        text.setPadding(s.dp(12), s.dp(10), s.dp(12), s.dp(18));
        TextView name = s.label(item.optString("title", ""), 15, TEXT);
        name.setMaxLines(2);
        name.setEllipsize(TextUtils.TruncateAt.END);
        text.addView(name);
        TextView line = s.label(join(" · ", item.optString("channel", ""), item.optString("views", ""),
            item.optString("published", "")), 12, SECONDARY);
        line.setMaxLines(2);
        line.setPadding(0, s.dp(4), 0, 0);
        text.addView(line);
        row.addView(text);
        return row;
    }

    private View commentRow(JSONObject comment) {
        LinearLayout row = new LinearLayout(activity);
        row.setPadding(s.dp(16), s.dp(12), s.dp(16), s.dp(8));
        ImageView picture = circle(32);
        RemoteImages.load(picture, comment.optString("avatar", ""), s.dp(32));
        row.addView(picture, new LinearLayout.LayoutParams(s.dp(32), s.dp(32)));
        LinearLayout body = new LinearLayout(activity);
        body.setOrientation(LinearLayout.VERTICAL);
        body.setPadding(s.dp(12), 0, 0, 0);
        if (comment.optBoolean("pinned")) {
            TextView pinned = s.label(s.text("Pinned"), 11, SECONDARY);
            pinned.setPadding(0, 0, 0, s.dp(2));
            body.addView(pinned);
        }
        TextView header = s.label(join(" • ", comment.optString("author", ""), comment.optString("published", "")),
            12, SECONDARY);
        if (comment.optBoolean("creator")) {
            header.setTextColor(TEXT);
        }
        body.addView(header);
        TextView text = s.label(comment.optString("text", ""), 14, TEXT);
        text.setLineSpacing(0, 1.1f);
        text.setPadding(0, s.dp(3), 0, 0);
        body.addView(text);
        LinearLayout actions = new LinearLayout(activity);
        actions.setGravity(Gravity.CENTER_VERTICAL);
        actions.setPadding(0, s.dp(6), 0, 0);
        TextView like = s.label(comment.optString("likes", ""), 12, SECONDARY);
        like.setCompoundDrawablesRelative(s.icon(PlayerIcons.LIKE, 14), null, null, null);
        like.setCompoundDrawablePadding(s.dp(6));
        actions.addView(like);
        String replies = comment.optString("replies", "");
        if (!replies.isEmpty() && !"0".equals(replies)) {
            TextView answers = s.label(replies + " " + s.text("replies"), 12, 0xFF3EA6FF);
            answers.setPadding(s.dp(20), 0, 0, 0);
            actions.addView(answers);
        }
        body.addView(actions);
        row.addView(body, new LinearLayout.LayoutParams(0, -2, 1));
        return row;
    }

    private TextView ratingHalf(String icon) {
        TextView view = s.label("", 14, TEXT);
        view.setTypeface(medium);
        view.setGravity(Gravity.CENTER_VERTICAL);
        view.setPadding(s.dp(14), 0, s.dp(14), 0);
        view.setCompoundDrawablesRelative(s.icon(icon, 20), null, null, null);
        view.setCompoundDrawablePadding(s.dp(6));
        return view;
    }

    private LinearLayout.LayoutParams pillParams() {
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-2, s.dp(36));
        params.setMarginStart(s.dp(8));
        return params;
    }

    private LinearLayout card() {
        LinearLayout card = new LinearLayout(activity);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setPadding(s.dp(12), s.dp(10), s.dp(12), s.dp(12));
        card.setBackground(new RippleDrawable(ColorStateList.valueOf(0x22FFFFFF),
            s.rounded(SURFACE, s.dp(12)), null));
        return card;
    }

    private LinearLayout.LayoutParams cardParams() {
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.setMargins(s.dp(12), s.dp(12), s.dp(12), 0);
        return params;
    }

    private ImageView circle(int sizeDp) {
        ImageView view = new ImageView(activity);
        view.setScaleType(ImageView.ScaleType.CENTER_CROP);
        GradientDrawable oval = new GradientDrawable();
        oval.setShape(GradientDrawable.OVAL);
        oval.setColor(SURFACE);
        view.setBackground(oval);
        view.setOutlineProvider(ViewOutlineProvider.BACKGROUND);
        view.setClipToOutline(true);
        return view;
    }

    private String compact(long value) {
        try {
            CompactDecimalFormat format = CompactDecimalFormat.getInstance(
                Locale.forLanguageTag(s.language), CompactDecimalFormat.CompactStyle.SHORT);
            return format.format(value);
        } catch (RuntimeException unavailable) {
            return String.valueOf(value);
        }
    }

    private static String join(String separator, String... parts) {
        StringBuilder out = new StringBuilder();
        for (String part : parts) {
            if (part == null || part.isEmpty()) {
                continue;
            }
            if (out.length() > 0) {
                out.append(separator);
            }
            out.append(part);
        }
        return out.toString();
    }

    /** A 16:9 image as wide as its parent. */
    private static final class RatioImage extends ImageView {
        RatioImage(Activity activity) {
            super(activity);
        }

        @Override protected void onMeasure(int widthSpec, int heightSpec) {
            int width = MeasureSpec.getSize(widthSpec);
            setMeasuredDimension(width, width * 9 / 16);
        }
    }
}
