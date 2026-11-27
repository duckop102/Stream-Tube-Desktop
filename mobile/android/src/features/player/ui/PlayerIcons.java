package io.github.shiroksh.streamtube;

import android.graphics.Canvas;
import android.graphics.ColorFilter;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.graphics.drawable.Drawable;

/** Material-style icons drawn from 24x24 path data, so the player needs no resources. */
final class PlayerIcons {
    static final String PLAY = "M8,5v14l11,-7z";
    static final String PAUSE = "M6,19h4V5H6v14zM14,5v14h4V5h-4z";
    static final String NEXT = "M6,18l8.5,-6L6,6v12zM16,6v12h2V6h-2z";
    static final String PREVIOUS = "M6,6h2v12H6zM9.5,12l8.5,6V6z";
    static final String REPLAY = "M12,5V1L7,6l5,5V7c3.31,0 6,2.69 6,6s-2.69,6 -6,6 -6,-2.69 -6,-6H4c0,4.42 3.58,8 8,8s8,-3.58 8,-8 -3.58,-8 -8,-8z";
    static final String FULLSCREEN = "M7,14H5v5h5v-2H7v-3zM5,10h2V7h3V5H5v5zM17,17h-3v2h5v-5h-2v3zM14,5v2h3v3h2V5h-5z";
    static final String FULLSCREEN_EXIT = "M5,16h3v3h2v-5H5v2zM8,8H5v2h5V5H8v3zM14,19h2v-3h3v-2h-5v5zM16,8V5h-2v5h5V8h-3z";
    static final String CLOSE = "M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12z";
    static final String CHEVRON_DOWN = "M16.59,8.59L12,13.17L7.41,8.59L6,10l6,6l6,-6z";
    static final String BACK = "M20,11H7.83l5.59,-5.59L12,4l-8,8l8,8l1.41,-1.41L7.83,13H20v-2z";
    static final String SETTINGS = "M19.14,12.94c0.04,-0.3 0.06,-0.61 0.06,-0.94c0,-0.32 -0.02,-0.64 -0.07,-0.94l2.03,-1.58"
        + "c0.18,-0.14 0.23,-0.41 0.12,-0.61l-1.92,-3.32c-0.12,-0.22 -0.37,-0.29 -0.59,-0.22l-2.39,0.96"
        + "c-0.5,-0.38 -1.03,-0.7 -1.62,-0.94L14.4,2.81c-0.04,-0.24 -0.24,-0.41 -0.48,-0.41h-3.84"
        + "c-0.24,0 -0.43,0.17 -0.47,0.41L9.25,5.35C8.66,5.59 8.12,5.92 7.63,6.29L5.24,5.33"
        + "c-0.22,-0.08 -0.47,0 -0.59,0.22L2.74,8.87C2.62,9.08 2.66,9.34 2.86,9.48l2.03,1.58"
        + "C4.84,11.36 4.8,11.69 4.8,12s0.02,0.64 0.07,0.94l-2.03,1.58c-0.18,0.14 -0.23,0.41 -0.12,0.61"
        + "l1.92,3.32c0.12,0.22 0.37,0.29 0.59,0.22l2.39,-0.96c0.5,0.38 1.03,0.7 1.62,0.94l0.36,2.54"
        + "c0.05,0.24 0.24,0.41 0.48,0.41h3.84c0.24,0 0.44,-0.17 0.47,-0.41l0.36,-2.54"
        + "c0.59,-0.24 1.13,-0.56 1.62,-0.94l2.39,0.96c0.22,0.08 0.47,0 0.59,-0.22l1.92,-3.32"
        + "c0.12,-0.22 0.07,-0.47 -0.12,-0.61L19.14,12.94z"
        + "M12,15.6c-1.98,0 -3.6,-1.62 -3.6,-3.6s1.62,-3.6 3.6,-3.6s3.6,1.62 3.6,3.6S13.98,15.6 12,15.6z";
    static final String LIKE = "M9,21h9c0.83,0 1.54,-0.5 1.84,-1.22l3.02,-7.05c0.09,-0.23 0.14,-0.47 0.14,-0.73v-2"
        + "c0,-1.1 -0.9,-2 -2,-2h-6.31l0.95,-4.57l0.03,-0.32c0,-0.41 -0.17,-0.79 -0.44,-1.06L14.17,1L7.58,7.59"
        + "C7.22,7.95 7,8.45 7,9v10c0,1.1 0.9,2 2,2zM9,9l4.34,-4.34L12,10h9v2l-3,7H9V9zM1,9h4v12H1z";
    static final String DISLIKE = "M15,3H6c-0.83,0 -1.54,0.5 -1.84,1.22l-3.02,7.05c-0.09,0.23 -0.14,0.47 -0.14,0.73v2"
        + "c0,1.1 0.9,2 2,2h6.31l-0.95,4.57l-0.03,0.32c0,0.41 0.17,0.79 0.44,1.06L9.83,23l6.59,-6.59"
        + "c0.36,-0.36 0.58,-0.86 0.58,-1.41V5c0,-1.1 -0.9,-2 -2,-2zM15,15l-4.34,4.34L12,14H3v-2l3,-7h9v10z"
        + "M19,3h4v12h-4z";
    static final String SHARE = "M14,9V5l7,7l-7,7v-4.1c-5,0 -8.5,1.6 -11,5.1c1,-5 4,-10 11,-11z";
    static final String SAVE = "M14,10H2v2h12v-2zM14,6H2v2h12V6zM18,14v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zM2,16h8v-2H2v2z";
    static final String SAVED = "M14,10H2v2h12v-2zM14,6H2v2h12V6zM2,16h8v-2H2v2zM21.5,11.5L23,13l-6.99,7l-4.51,-4.5L13,14l3.01,3l5.49,-5.5z";
    static final String REWIND = "M11,18V6l-8.5,6l8.5,6zM11.5,12l8.5,6V6l-8.5,6z";
    static final String FORWARD = "M4,18l8.5,-6L4,6v12zM13,6v12l8.5,-6L13,6z";
    static final String COMMENT = "M21.99,4c0,-1.1 -0.89,-2 -1.99,-2H4c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h14l4,4l-0.01,-18z"
        + "M20,4v13.17L18.83,16H4V4h16z";

    private PlayerIcons() {}

    static Drawable of(String data, int color) {
        return new Icon(parse(data), color);
    }

    private static final class Icon extends Drawable {
        private final Path path;
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Path scaled = new Path();
        private final android.graphics.Matrix matrix = new android.graphics.Matrix();

        Icon(Path path, int color) {
            this.path = path;
            paint.setColor(color);
            paint.setStyle(Paint.Style.FILL);
        }

        @Override public void draw(Canvas canvas) {
            Rect bounds = getBounds();
            if (bounds.isEmpty()) {
                return;
            }
            matrix.setScale(bounds.width() / 24f, bounds.height() / 24f);
            matrix.postTranslate(bounds.left, bounds.top);
            path.transform(matrix, scaled);
            canvas.drawPath(scaled, paint);
        }

        @Override public void setAlpha(int alpha) {
            paint.setAlpha(alpha);
            invalidateSelf();
        }

        @Override public void setColorFilter(ColorFilter filter) {
            paint.setColorFilter(filter);
            invalidateSelf();
        }

        @Override public int getOpacity() {
            return PixelFormat.TRANSLUCENT;
        }
    }

    /** SVG path data with M, L, H, V, C, S, Q, T and Z commands (absolute and relative). */
    static Path parse(String data) {
        Path path = new Path();
        path.setFillType(Path.FillType.EVEN_ODD);
        Reader in = new Reader(data);
        float x = 0, y = 0, startX = 0, startY = 0, controlX = 0, controlY = 0;
        char command = 'M';
        char previous = ' ';
        while (in.more()) {
            char next = in.peekCommand();
            if (next != 0) {
                command = next;
                in.position++;
            } else if (command == 'M') {
                command = 'L';
            } else if (command == 'm') {
                command = 'l';
            }
            boolean relative = Character.isLowerCase(command);
            float baseX = relative ? x : 0;
            float baseY = relative ? y : 0;
            switch (Character.toUpperCase(command)) {
                case 'M':
                    x = baseX + in.number();
                    y = baseY + in.number();
                    path.moveTo(x, y);
                    startX = x;
                    startY = y;
                    break;
                case 'L':
                    x = baseX + in.number();
                    y = baseY + in.number();
                    path.lineTo(x, y);
                    break;
                case 'H':
                    x = baseX + in.number();
                    path.lineTo(x, y);
                    break;
                case 'V':
                    y = baseY + in.number();
                    path.lineTo(x, y);
                    break;
                case 'C': {
                    float x1 = baseX + in.number(), y1 = baseY + in.number();
                    controlX = baseX + in.number();
                    controlY = baseY + in.number();
                    x = baseX + in.number();
                    y = baseY + in.number();
                    path.cubicTo(x1, y1, controlX, controlY, x, y);
                    break;
                }
                case 'S': {
                    boolean smooth = "CcSs".indexOf(previous) >= 0;
                    float x1 = smooth ? 2 * x - controlX : x;
                    float y1 = smooth ? 2 * y - controlY : y;
                    controlX = baseX + in.number();
                    controlY = baseY + in.number();
                    x = baseX + in.number();
                    y = baseY + in.number();
                    path.cubicTo(x1, y1, controlX, controlY, x, y);
                    break;
                }
                case 'Q':
                    controlX = baseX + in.number();
                    controlY = baseY + in.number();
                    x = baseX + in.number();
                    y = baseY + in.number();
                    path.quadTo(controlX, controlY, x, y);
                    break;
                case 'T': {
                    boolean smooth = "QqTt".indexOf(previous) >= 0;
                    controlX = smooth ? 2 * x - controlX : x;
                    controlY = smooth ? 2 * y - controlY : y;
                    x = baseX + in.number();
                    y = baseY + in.number();
                    path.quadTo(controlX, controlY, x, y);
                    break;
                }
                case 'Z':
                    path.close();
                    x = startX;
                    y = startY;
                    break;
                default:
                    throw new IllegalArgumentException("Unsupported path command " + command);
            }
            previous = command;
        }
        return path;
    }

    private static final class Reader {
        final String data;
        int position;

        Reader(String data) {
            this.data = data;
        }

        boolean more() {
            skip();
            return position < data.length();
        }

        char peekCommand() {
            skip();
            char c = data.charAt(position);
            return Character.isLetter(c) && c != 'e' && c != 'E' ? c : 0;
        }

        void skip() {
            while (position < data.length()) {
                char c = data.charAt(position);
                if (c != ' ' && c != ',' && c != '\n' && c != '\t') {
                    break;
                }
                position++;
            }
        }

        float number() {
            skip();
            int start = position;
            if (position < data.length() && (data.charAt(position) == '-' || data.charAt(position) == '+')) {
                position++;
            }
            boolean dot = false;
            while (position < data.length()) {
                char c = data.charAt(position);
                if (c >= '0' && c <= '9') {
                    position++;
                } else if (c == '.' && !dot) {
                    dot = true;
                    position++;
                } else if ((c == 'e' || c == 'E') && position > start) {
                    position++;
                    if (position < data.length() && (data.charAt(position) == '-' || data.charAt(position) == '+')) {
                        position++;
                    }
                } else {
                    break;
                }
            }
            if (start == position) {
                throw new IllegalArgumentException("Expected a number at " + start);
            }
            return Float.parseFloat(data.substring(start, position));
        }
    }
}
