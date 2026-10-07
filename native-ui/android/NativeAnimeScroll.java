package app.rustdl;

import android.view.MotionEvent;
import android.view.VelocityTracker;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.OverScroller;

/** Anime-only pans: Android event timestamps determine release velocity. */
final class NativeAnimeScroll {
    private final View view;
    private final OverScroller fling;
    private final float density;
    private final int slop, minimum, maximum;
    private VelocityTracker velocity;
    private long epoch;
    private int pointer, lastFlingY;
    private float startX, startY, lastY;
    private long lastMoveTime;
    private boolean dragging, tracking;
    private final Runnable frame = new Runnable() {
        public void run() {
            if (epoch == 0 || epoch != NativeHomeHost.nativeAnimeScrollEpoch()) { stop(); return; }
            if (!fling.computeScrollOffset()) return;
            int next = fling.getCurrY();
            NativeHomeHost.nativeAnimeScroll(epoch, (next - lastFlingY) / density);
            lastFlingY = next;
            if (!fling.isFinished()) view.postOnAnimation(this);
        }
    };
    NativeAnimeScroll(View view) {
        this.view = view;
        density = view.getResources().getDisplayMetrics().density;
        ViewConfiguration configuration = ViewConfiguration.get(view.getContext());
        slop = configuration.getScaledTouchSlop();
        minimum = configuration.getScaledMinimumFlingVelocity();
        maximum = configuration.getScaledMaximumFlingVelocity();
        fling = new OverScroller(view.getContext());
    }
    void stop() {
        view.removeCallbacks(frame);
        fling.abortAnimation();
        if (epoch != 0) NativeHomeHost.nativeStopAnimeScroll(epoch);
        epoch = 0; tracking = false; dragging = false;
        if (velocity != null) { velocity.recycle(); velocity = null; }
    }
    boolean touch(MotionEvent event) {
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_DOWN) {
            stop(); epoch = NativeHomeHost.nativeAnimeScrollEpoch();
            if (epoch == 0) return false;
            pointer = event.getPointerId(0);
            startX = event.getX(); startY = lastY = event.getY();
            lastMoveTime = event.getEventTime(); tracking = true; velocity = VelocityTracker.obtain(); velocity.addMovement(event);
            return false; // GPUI retains taps until a vertical pan wins.
        }
        if (!tracking) return false;
        if (epoch != NativeHomeHost.nativeAnimeScrollEpoch() || event.getPointerCount() != 1
                || action == MotionEvent.ACTION_CANCEL) {
            boolean consumed = dragging;
            if (consumed) NativeHomeHost.nativeTouch(MotionEvent.ACTION_CANCEL, pointer, startX, lastY);
            stop(); return consumed;
        }
        velocity.addMovement(event);
        if (action == MotionEvent.ACTION_MOVE) {
            float y = event.getY();
            if (!dragging) {
                float dx = event.getX() - startX, dy = y - startY;
                if (Math.abs(dy) <= slop || Math.abs(dy) < Math.abs(dx)) return false;
                NativeHomeHost.nativeTouch(MotionEvent.ACTION_CANCEL, pointer, startX, startY);
                dragging = true;
            }
            NativeHomeHost.nativeAnimeScroll(epoch, (y - lastY) / density);
            lastY = y; lastMoveTime = event.getEventTime();
            return true;
        }
        if (action == MotionEvent.ACTION_UP) {
            boolean consumed = dragging;
            if (consumed) {
                NativeHomeHost.nativeAnimeScroll(epoch, (event.getY() - lastY) / density);
                velocity.computeCurrentVelocity(1000, maximum);
                float speed = velocity.getYVelocity(pointer);
                // A held finger must stop rather than fling using an old move.
                if (event.getEventTime() - lastMoveTime > 40) speed = 0;
                if (Math.abs(speed) >= minimum) {
                    lastFlingY = 0;
                    fling.fling(0, 0, 0, (int) speed, 0, 0, -1000000, 1000000);
                    view.postOnAnimation(frame);
                }
            }
            tracking = false; dragging = false;
            velocity.recycle(); velocity = null;
            return consumed;
        }
        return dragging;
    }
}
