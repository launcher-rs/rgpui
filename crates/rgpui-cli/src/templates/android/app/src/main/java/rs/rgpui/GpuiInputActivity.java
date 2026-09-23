package rs.rgpui;

import android.app.NativeActivity;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.BatteryManager;
import android.view.ViewGroup;
import android.view.inputmethod.InputMethodManager;

/**
 * 带输入法的 NativeActivity：挂载 {@link GpuiInputView} 提供组合串通道。
 *
 * <p>包名固定 `rs.rgpui`（与应用 `applicationId` 无关），Rust 侧 JNI 入口
 * 按此包名拼接（见 `rgpui-android/src/ime.rs`）。注意：`onCreateInputConnection`
 * 是 `View` 的方法，Activity 本身没有——连接必须由聚焦的 View 提供，
 * 这里复用一个 1px 的 {@link GpuiInputView}（Godot/SDL 同款思路）。
 */
public class GpuiInputActivity extends NativeActivity {
    static {
        // framework 经 loadNativeCode 直载 .so，不走 ClassLoader 登记，
        // JNI 按名解析看不见符号；这里再 load 一次（已加载只做登记，不重复映射）。
        System.loadLibrary("__RGPUI_LIB_NAME__");
    }

    private GpuiInputView inputView;

    /** 电池广播接收器：电量/充电状态一变就推 Rust（界面实时刷新）。 */
    private final BroadcastReceiver batteryReceiver =
            new BroadcastReceiver() {
                @Override
                public void onReceive(Context context, Intent intent) {
                    int level =
                            intent.getIntExtra(BatteryManager.EXTRA_LEVEL, -1);
                    int scale =
                            intent.getIntExtra(BatteryManager.EXTRA_SCALE, -1);
                    int status =
                            intent.getIntExtra(BatteryManager.EXTRA_STATUS, -1);
                    nativeBatteryChanged(level, scale, status);
                }
            };

    @Override
    protected void onResume() {
        super.onResume();
        // 粘性广播：注册即回最后一次状态，之后有变再推。
        registerReceiver(
                batteryReceiver, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
    }

    @Override
    protected void onPause() {
        unregisterReceiver(batteryReceiver);
        super.onPause();
    }

    /** 取输入视图（没有就挂一个 1px 的到 content，不挡 GL 渲染）。 */
    private GpuiInputView ensureInputView() {
        if (inputView == null) {
            inputView = new GpuiInputView(this);
            ViewGroup content =
                    (ViewGroup)
                            getWindow()
                                    .getDecorView()
                                    .findViewById(android.R.id.content);
            content.addView(
                    inputView,
                    new ViewGroup.LayoutParams(1, 1));
        }
        return inputView;
    }

    /** 弹软键盘（Rust 侧调；切 UI 线程跑 View 操作）。 */
    public void showKeyboard() {
        runOnUiThread(
                () -> {
                    GpuiInputView view = ensureInputView();
                    view.requestFocus();
                    InputMethodManager imm =
                            (InputMethodManager)
                                    getSystemService(Context.INPUT_METHOD_SERVICE);
                    if (imm != null) {
                        imm.showSoftInput(view, 0);
                    }
                });
    }

    /** 收软键盘（Rust 侧调；切 UI 线程跑 View 操作）。 */
    public void hideKeyboard() {
        runOnUiThread(
                () -> {
                    GpuiInputView view = ensureInputView();
                    InputMethodManager imm =
                            (InputMethodManager)
                                    getSystemService(Context.INPUT_METHOD_SERVICE);
                    if (imm != null) {
                        imm.hideSoftInputFromWindow(view.getWindowToken(), 0);
                    }
                    view.clearFocus();
                });
    }

    private static native void nativeBatteryChanged(int level, int scale, int status);
}
