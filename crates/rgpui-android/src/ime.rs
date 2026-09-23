//! 输入法（IME）组合串：自定义 Activity 的 `InputConnection` 入口。
//!
//! 线程模型（见 `bridge` 模块头死锁说明）：
//! * Java 侧（UI 线程）调进来的 JNI 函数**只入队、不拿任何窗口锁**，
//!   入队后叫醒主循环即返回；查询类调用（`getTextBeforeCursor` 等）
//!   现阶段直接回空（M3-2b 再接快照）。
//! * 主循环（native 线程，`bridge::drain_ime_ops`）逐个取出，经
//!   `AndroidWindow::handle_ime` → `input_callback` 交核心
//!   `Window::dispatch_ime_event`（take/restore 输入处理器，与 macOS 同模式）。
//!
//! 本模块仅 Android 编译（`#[cfg(target_os = "android")]` 门控），
//! Java 对端见 `examples/hello_mobile/android/.../rs/rgpui/`。

use parking_lot::Mutex;
use rgpui::ImeEvent;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, Ordering};

/// 待消化的 IME 操作队列（Java UI 线程生产，主循环消费）。
static IME_OPS: Mutex<VecDeque<ImeEvent>> = Mutex::new(VecDeque::new());

/// 当前键盘类型（`show_keyboard_with_type` 存，`EditorInfo` 经 JNI 来取）。
static INPUT_TYPE: AtomicU8 = AtomicU8::new(KeyboardTypeTag::Default as u8);

/// 与 [`super::KeyboardType`] 一一对应的可存标签（`AtomicU8` 存盘符）。
#[derive(Clone, Copy)]
enum KeyboardTypeTag {
    Default = 0,
    EmailAddress = 1,
    Phone = 2,
    NumberPad = 3,
    Url = 4,
    Decimal = 5,
}

/// 记下调用方想要的键盘类型（真机下次弹键盘时经 `EditorInfo` 生效）。
pub fn set_input_type(keyboard_type: super::KeyboardType) {
    let tag = match keyboard_type {
        super::KeyboardType::Default => KeyboardTypeTag::Default,
        super::KeyboardType::EmailAddress => KeyboardTypeTag::EmailAddress,
        super::KeyboardType::Phone => KeyboardTypeTag::Phone,
        super::KeyboardType::NumberPad => KeyboardTypeTag::NumberPad,
        super::KeyboardType::URL => KeyboardTypeTag::Url,
        super::KeyboardType::Decimal => KeyboardTypeTag::Decimal,
    };
    INPUT_TYPE.store(tag as u8, Ordering::Relaxed);
}

/// 当前键盘类型对应的 Android `InputType` 整型（供 Java `EditorInfo` 用）。
///
/// 常量：`TYPE_CLASS_TEXT = 1`，`TYPE_TEXT_VARIATION_EMAIL_ADDRESS = 32`，
/// `TYPE_CLASS_PHONE = 3`，`TYPE_CLASS_NUMBER = 2`，
/// `TYPE_TEXT_VARIATION_URI = 16`，`TYPE_NUMBER_FLAG_DECIMAL = 8192`。
pub fn input_type_int() -> i32 {
    match INPUT_TYPE.load(Ordering::Relaxed) {
        1 => 1 | 32,
        2 => 3,
        3 => 2,
        4 => 1 | 16,
        5 => 2 | 8192,
        _ => 1,
    }
}

/// 入队一个 IME 操作并叫醒主循环（Java UI 线程调，无锁等待）。
pub(crate) fn push_ime_op(event: ImeEvent) {
    IME_OPS.lock().push_back(event);
    super::mark_text_input_dirty();
}

/// 取出全部待消化操作（主循环调）。
pub(crate) fn take_ime_ops() -> Vec<ImeEvent> {
    IME_OPS.lock().drain(..).collect()
}

// ── JNI 入口（Java `GpuiInputConnection` / `GpuiInputActivity` 调） ─────────────
// 包名固定 `rs.rgpui`（与应用 `applicationId` 无关），函数名按 JNI 规范拼接。
// 一律经 `run_jni`（异常转日志 + panic 截获），只入队即返回。

/// JNI 字符串转 Rust 字符串（空指针给空串）。
fn jstring_to_string(env: &mut jni::Env<'_>, raw: jni::sys::jstring) -> String {
    if raw.is_null() {
        return String::new();
    }
    let obj = unsafe { jni::objects::JObject::from_raw(env, raw as jni::sys::jobject) };
    super::bridge::get_string(env, &obj)
}

/// 提交确定文本。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_rs_rgpui_GpuiInputConnection_nativeCommitText(
    raw_env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    text: jni::sys::jstring,
) {
    super::bridge::run_jni(raw_env, |env| {
        let text = jstring_to_string(env, text);
        if !text.is_empty() {
            push_ime_op(ImeEvent::Commit(text));
        }
    });
}

/// 设置组合串（`new_cursor` 照 Android `setComposingText` 语义透传，core 侧换算）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_rs_rgpui_GpuiInputConnection_nativeSetComposingText(
    raw_env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    text: jni::sys::jstring,
    new_cursor: jni::sys::jint,
) {
    super::bridge::run_jni(raw_env, |env| {
        let text = jstring_to_string(env, text);
        push_ime_op(ImeEvent::SetComposing {
            text,
            cursor: new_cursor,
        });
    });
}

/// 结束组合（确认当前组合串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_rs_rgpui_GpuiInputConnection_nativeFinishComposing(
    raw_env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
) {
    super::bridge::run_jni(raw_env, |_env| {
        push_ime_op(ImeEvent::FinishComposing);
    });
}

/// 删除光标前后文本（Java 侧 UTF-16 计数，core 侧超界钳制）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_rs_rgpui_GpuiInputConnection_nativeDeleteSurroundingText(
    raw_env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    before: jni::sys::jint,
    after: jni::sys::jint,
) {
    super::bridge::run_jni(raw_env, |_env| {
        push_ime_op(ImeEvent::DeleteSurrounding { before, after });
    });
}

/// 输入法按键（只处理退格/回车，其余走 NDK 按键通道不管）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_rs_rgpui_GpuiInputConnection_nativeSendKeyEvent(
    raw_env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    key_code: jni::sys::jint,
    _action: jni::sys::jint,
) {
    super::bridge::run_jni(raw_env, |_env| match key_code {
        super::keyboard::AKEYCODE_DEL => {
            push_ime_op(ImeEvent::DeleteSurrounding {
                before: 1,
                after: 0,
            });
        }
        super::keyboard::AKEYCODE_ENTER => {
            push_ime_op(ImeEvent::Commit("\n".to_string()));
        }
        _ => {}
    });
}

/// 编辑器动作（搜索/发送/完成等收键盘；未指定则提交换行）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_rs_rgpui_GpuiInputConnection_nativePerformEditorAction(
    raw_env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    action: jni::sys::jint,
) {
    super::bridge::run_jni(raw_env, |_env| {
        // `EditorInfo.IME_ACTION_UNSPECIFIED = 0`：提交换行；其余一律收键盘。
        if action == 0 {
            push_ime_op(ImeEvent::Commit("\n".to_string()));
        } else {
            super::bridge::hide_keyboard_android();
        }
    });
}

/// 取当前键盘类型的 `InputType` 整型（`GpuiInputView.onCreateInputConnection` 用）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_rs_rgpui_GpuiInputView_nativeGetInputType(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
) -> jni::sys::jint {
    // 纯读原子量，不碰 JNI 环境，直接返回。
    input_type_int()
}
