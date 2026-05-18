//! XIM（X Input Method）输入法处理器模块
//!
//! 处理 X11 输入法框架的回调事件，包括预编辑和提交

use std::default::Default;

use x11rb::protocol::{Event, xproto};
use xim::{AHashMap, AttributeName, Client, ClientError, ClientHandler, InputStyle};

/// XIM 回调事件类型
pub enum XimCallbackEvent {
    /// X11 原生事件
    XimXEvent(x11rb::protocol::Event),
    /// 预编辑事件
    XimPreeditEvent(xproto::Window, String),
    /// 提交事件
    XimCommitEvent(xproto::Window, String),
}

/// XIM 处理器实现
pub struct XimHandler {
    /// 输入法 ID
    pub im_id: u16,
    /// 输入上下文 ID
    pub ic_id: u16,
    /// 是否已连接
    pub connected: bool,
    /// 关联的窗口
    pub window: xproto::Window,
    /// 最后的回调事件
    pub last_callback_event: Option<XimCallbackEvent>,
}

impl XimHandler {
    /// 创建新的 XIM 处理器实例
    pub fn new() -> Self {
        Self {
            im_id: Default::default(),
            ic_id: Default::default(),
            connected: false,
            window: Default::default(),
            last_callback_event: None,
        }
    }
}

impl<C: Client<XEvent = xproto::KeyPressEvent>> ClientHandler<C> for XimHandler {
    fn handle_connect(&mut self, client: &mut C) -> Result<(), ClientError> {
        client.open("C")
    }

    fn handle_open(&mut self, client: &mut C, input_method_id: u16) -> Result<(), ClientError> {
        self.im_id = input_method_id;

        client.get_im_values(input_method_id, &[AttributeName::QueryInputStyle])
    }

    fn handle_get_im_values(
        &mut self,
        client: &mut C,
        input_method_id: u16,
        _attributes: AHashMap<AttributeName, Vec<u8>>,
    ) -> Result<(), ClientError> {
        let ic_attributes = client
            .build_ic_attributes()
            .push(AttributeName::InputStyle, InputStyle::PREEDIT_CALLBACKS)
            .push(AttributeName::ClientWindow, self.window)
            .push(AttributeName::FocusWindow, self.window)
            .build();
        client.create_ic(input_method_id, ic_attributes)
    }

    fn handle_create_ic(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        self.connected = true;
        self.ic_id = input_context_id;
        Ok(())
    }

    fn handle_commit(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
        text: &str,
    ) -> Result<(), ClientError> {
        self.last_callback_event = Some(XimCallbackEvent::XimCommitEvent(
            self.window,
            String::from(text),
        ));
        Ok(())
    }

    fn handle_forward_event(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
        _flag: xim::ForwardEventFlag,
        xev: C::XEvent,
    ) -> Result<(), ClientError> {
        match xev.response_type {
            x11rb::protocol::xproto::KEY_PRESS_EVENT => {
                self.last_callback_event = Some(XimCallbackEvent::XimXEvent(Event::KeyPress(xev)));
            }
            x11rb::protocol::xproto::KEY_RELEASE_EVENT => {
                self.last_callback_event =
                    Some(XimCallbackEvent::XimXEvent(Event::KeyRelease(xev)));
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_close(&mut self, client: &mut C, _input_method_id: u16) -> Result<(), ClientError> {
        client.disconnect()
    }

    fn handle_preedit_draw(
        &mut self,
        _client: &mut C,
        _input_method_id: u16,
        _input_context_id: u16,
        _caret: i32,
        _chg_first: i32,
        _chg_len: i32,
        _status: xim::PreeditDrawStatus,
        preedit_string: &str,
        _feedbacks: Vec<xim::Feedback>,
    ) -> Result<(), ClientError> {
        // XIMReverse: 1, XIMPrimary: 8, XIMTertiary: 32: 选中文本
        // XIMUnderline: 2, XIMSecondary: 16: 带下划线的文本
        // XIMHighlight: 4: 普通文本
        // XIMVisibleToForward: 64, XIMVisibleToBackward: 128, XIMVisibleCenter: 256: 文本对齐位置
        // XIMPrimary, XIMHighlight, XIMSecondary, XIMTertiary 未指定，
        // 但可如上互换使用
        // 目前无法支持这些
        self.last_callback_event = Some(XimCallbackEvent::XimPreeditEvent(
            self.window,
            String::from(preedit_string),
        ));
        Ok(())
    }
}
