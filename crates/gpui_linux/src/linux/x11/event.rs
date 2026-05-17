use x11rb::protocol::{
    xinput,
    xproto::{self, ModMask},
};

use gpui::{Modifiers, MouseButton, NavigationDirection};

/// X11 按钮或滚动事件
pub(crate) enum ButtonOrScroll {
    /// 鼠标按钮
    Button(MouseButton),
    /// 滚动方向
    Scroll(ScrollDirection),
}

/// 滚动方向
pub(crate) enum ScrollDirection {
    /// 向上滚动
    Up,
    /// 向下滚动
    Down,
    /// 向左滚动
    Left,
    /// 向右滚动
    Right,
}

/// 从事件详情转换为按钮或滚动
pub(crate) fn button_or_scroll_from_event_detail(detail: u32) -> Option<ButtonOrScroll> {
    Some(match detail {
        1 => ButtonOrScroll::Button(MouseButton::Left),
        2 => ButtonOrScroll::Button(MouseButton::Middle),
        3 => ButtonOrScroll::Button(MouseButton::Right),
        4 => ButtonOrScroll::Scroll(ScrollDirection::Up),
        5 => ButtonOrScroll::Scroll(ScrollDirection::Down),
        6 => ButtonOrScroll::Scroll(ScrollDirection::Left),
        7 => ButtonOrScroll::Scroll(ScrollDirection::Right),
        8 => ButtonOrScroll::Button(MouseButton::Navigate(NavigationDirection::Back)),
        9 => ButtonOrScroll::Button(MouseButton::Navigate(NavigationDirection::Forward)),
        _ => return None,
    })
}

pub(crate) fn modifiers_from_state(state: xproto::KeyButMask) -> Modifiers {
    Modifiers {
        control: state.contains(xproto::KeyButMask::CONTROL),
        alt: state.contains(xproto::KeyButMask::MOD1),
        shift: state.contains(xproto::KeyButMask::SHIFT),
        platform: state.contains(xproto::KeyButMask::MOD4),
        function: false,
    }
}

pub(crate) fn modifiers_from_xinput_info(modifier_info: xinput::ModifierInfo) -> Modifiers {
    Modifiers {
        control: modifier_info.effective as u16 & ModMask::CONTROL.bits()
            == ModMask::CONTROL.bits(),
        alt: modifier_info.effective as u16 & ModMask::M1.bits() == ModMask::M1.bits(),
        shift: modifier_info.effective as u16 & ModMask::SHIFT.bits() == ModMask::SHIFT.bits(),
        platform: modifier_info.effective as u16 & ModMask::M4.bits() == ModMask::M4.bits(),
        function: false,
    }
}

pub(crate) fn pressed_button_from_mask(button_mask: u32) -> Option<MouseButton> {
    Some(if button_mask & 2 == 2 {
        MouseButton::Left
    } else if button_mask & 4 == 4 {
        MouseButton::Middle
    } else if button_mask & 8 == 8 {
        MouseButton::Right
    } else {
        return None;
    })
}

/// 获取 XInput 事件的 valuator 轴索引
///
/// XInput valuator 掩码在此事件 axisvalues 中存在的每个 valuator 对应的位索引处有 1
/// Axisvalues 从最低 valuator 编号到最高编号排序，
/// 因此计算此 valuator 之前的位数可得出 axisvalues 中的索引
pub(crate) fn get_valuator_axis_index(
    valuator_mask: &Vec<u32>,
    valuator_number: u16,
) -> Option<usize> {
    if bit_is_set_in_vec(valuator_mask, valuator_number) {
        Some(popcount_upto_bit_index(valuator_mask, valuator_number) as usize)
    } else {
        None
    }
}

/// 返回 `bit_vec` 中所有 `i < bit_index` 位的 1 的数量
fn popcount_upto_bit_index(bit_vec: &Vec<u32>, bit_index: u16) -> u32 {
    let array_index = bit_index as usize / 32;
    let popcount: u32 = bit_vec
        .get(array_index)
        .map_or(0, |bits| keep_bits_upto(*bits, bit_index % 32).count_ones());
    if array_index == 0 {
        popcount
    } else {
        // 滚动位置的 valuator 编号超过 32 可能永远不会出现，但最好还是支持它
        let leading_popcount: u32 = bit_vec
            .iter()
            .take(array_index)
            .map(|bits| bits.count_ones())
            .sum();
        popcount + leading_popcount
    }
}

/// 检查位向量中指定索引的位是否已设置
fn bit_is_set_in_vec(bit_vec: &Vec<u32>, bit_index: u16) -> bool {
    let array_index = bit_index as usize / 32;
    bit_vec
        .get(array_index)
        .is_some_and(|bits| bit_is_set(*bits, bit_index % 32))
}

/// 检查指定位是否已设置
fn bit_is_set(bits: u32, bit_index: u16) -> bool {
    bits & (1 << bit_index) != 0
}

/// 将每个 `i >= bit_index` 的位设置为 0
fn keep_bits_upto(bits: u32, bit_index: u16) -> u32 {
    if bit_index == 0 {
        0
    } else if bit_index >= 32 {
        u32::MAX
    } else {
        bits & ((1 << bit_index) - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_valuator_axis_index() {
        assert!(get_valuator_axis_index(&vec![0b11], 0) == Some(0));
        assert!(get_valuator_axis_index(&vec![0b11], 1) == Some(1));
        assert!(get_valuator_axis_index(&vec![0b11], 2) == None);

        assert!(get_valuator_axis_index(&vec![0b100], 0) == None);
        assert!(get_valuator_axis_index(&vec![0b100], 1) == None);
        assert!(get_valuator_axis_index(&vec![0b100], 2) == Some(0));
        assert!(get_valuator_axis_index(&vec![0b100], 3) == None);

        assert!(get_valuator_axis_index(&vec![0b1010, 0], 0) == None);
        assert!(get_valuator_axis_index(&vec![0b1010, 0], 1) == Some(0));
        assert!(get_valuator_axis_index(&vec![0b1010, 0], 2) == None);
        assert!(get_valuator_axis_index(&vec![0b1010, 0], 3) == Some(1));

        assert!(get_valuator_axis_index(&vec![0b1010, 0b1], 0) == None);
        assert!(get_valuator_axis_index(&vec![0b1010, 0b1], 1) == Some(0));
        assert!(get_valuator_axis_index(&vec![0b1010, 0b1], 2) == None);
        assert!(get_valuator_axis_index(&vec![0b1010, 0b1], 3) == Some(1));
        assert!(get_valuator_axis_index(&vec![0b1010, 0b1], 32) == Some(2));
        assert!(get_valuator_axis_index(&vec![0b1010, 0b1], 33) == None);

        assert!(get_valuator_axis_index(&vec![0b1010, 0b101], 34) == Some(3));
    }
}
