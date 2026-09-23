package rs.rgpui;

import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;

/**
 * 输入法连接：提交方向透传给 Rust，主线程只入队不阻塞。
 *
 * <p>查询方向（光标前后文本/选区/全文）现阶段回空（M3-2b 接快照），
 * 拼音组词提交不受影响。所有 native 方法都在 Rust 侧
 * `rgpui-android/src/ime.rs` 实现，UI 线程只入队即返回。
 */
public class GpuiInputConnection extends BaseInputConnection {
    public GpuiInputConnection(View targetView, boolean fullEditor) {
        super(targetView, fullEditor);
    }

    @Override
    public boolean commitText(CharSequence text, int newCursorPosition) {
        nativeCommitText(text.toString());
        return true;
    }

    @Override
    public boolean setComposingText(CharSequence text, int newCursorPosition) {
        nativeSetComposingText(text.toString(), newCursorPosition);
        return true;
    }

    @Override
    public boolean finishComposingText() {
        nativeFinishComposing();
        return true;
    }

    @Override
    public boolean deleteSurroundingText(int beforeLength, int afterLength) {
        nativeDeleteSurroundingText(beforeLength, afterLength);
        return true;
    }

    @Override
    public boolean deleteSurroundingTextInCodePoints(int beforeLength, int afterLength) {
        nativeDeleteSurroundingText(beforeLength, afterLength);
        return true;
    }

    @Override
    public boolean sendKeyEvent(KeyEvent event) {
        nativeSendKeyEvent(event.getKeyCode(), event.getAction());
        return true;
    }

    @Override
    public boolean performEditorAction(int actionCode) {
        nativePerformEditorAction(actionCode);
        return true;
    }

    @Override
    public boolean setSelection(int start, int end) {
        // M3-2b 随文本快照实现，现阶段忽略。
        return true;
    }

    @Override
    public CharSequence getTextBeforeCursor(int length, int flags) {
        return null;
    }

    @Override
    public CharSequence getTextAfterCursor(int length, int flags) {
        return null;
    }

    @Override
    public CharSequence getSelectedText(int flags) {
        return null;
    }

    @Override
    public ExtractedText getExtractedText(ExtractedTextRequest request, int flags) {
        return null;
    }

    @Override
    public boolean beginBatchEdit() {
        return true;
    }

    @Override
    public boolean endBatchEdit() {
        return true;
    }

    @Override
    public boolean reportFullscreenMode(boolean enabled) {
        return false;
    }

    @Override
    public boolean clearMetaKeyStates(int states) {
        return false;
    }

    private static native void nativeCommitText(String text);

    private static native void nativeSetComposingText(String text, int newCursor);

    private static native void nativeFinishComposing();

    private static native void nativeDeleteSurroundingText(int beforeLength, int afterLength);

    private static native void nativeSendKeyEvent(int keyCode, int action);

    private static native void nativePerformEditorAction(int actionCode);
}
