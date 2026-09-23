package rs.rgpui;

import android.content.Context;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;

/**
 * 输入法宿主视图：1px 占位，向输入法提供 {@link GpuiInputConnection}。
 *
 * <p>`onCheckIsTextEditor() == true` 是输入法绑定的前提；
 * `onCreateInputConnection` 里按 Rust 侧键盘类型填 `EditorInfo`。
 */
public class GpuiInputView extends View {
    private GpuiInputConnection inputConnection;

    public GpuiInputView(Context context) {
        super(context);
        setFocusable(true);
        setFocusableInTouchMode(true);
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
        // 输入类型由 Rust 侧键盘类型决定（`show_keyboard_with_type` 存盘）。
        outAttrs.inputType = nativeGetInputType();
        outAttrs.imeOptions =
                EditorInfo.IME_ACTION_UNSPECIFIED | EditorInfo.IME_FLAG_NO_FULLSCREEN;
        outAttrs.initialCapsMode = 0;
        if (inputConnection == null) {
            inputConnection = new GpuiInputConnection(this, true);
        }
        return inputConnection;
    }

    private static native int nativeGetInputType();
}
