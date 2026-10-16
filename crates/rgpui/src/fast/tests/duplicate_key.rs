//! 同键重复请求的元素级回归（1.4 保留式布局缺陷）。
//!
//! 同一帧里两处元素共享同一个 [`ElementId`] 时（最常见的是把默认 id 取自调用点的
//! 组件放进循环、又没显式传 id），元素树里就会出现两条完全相同的布局路径。
//! 旧实现把这种「同帧第二次请求」做成临时节点并与第一次共享子节点：
//! `taffy::new_with_children` 会把子节点的 parent 指针改指到临时节点，而第一次的
//! 父节点 `children` 列表里仍留着这个子节点（Taffy 里一个节点只能有一个父节点）。
//! 结果先表现为 `layout_bounds` 把这个子节点当成根节点、子树整体错位，之后在帧末
//! 清理节点时索引到已失效的 SlotMap 键而 panic（`invalid SlotMap key used`）。
//!
//! 本测试锁定三件事：多帧绘制不得 panic；场景必须逐帧一致；被复制的那两个子元素
//! 仍停在自己父元素下方（0 与 40），且第二帧确实命中了保留复用。

use crate::{
    AppContext, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    TestAppContext, Window, div, px, rgba, size, v_flex,
};

/// 回归用根视图：两个同 id 的兄弟，各带一个带背景的子元素。
///
/// 视图每帧都会重新渲染并重新请求布局（见 `ViewElement::request_layout` 的说明），
/// 所以这条重复路径在每一帧都会被走到。
struct DuplicateRoot;

impl Render for DuplicateRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .child(
                div()
                    .id(ElementId::Name("dup".into()))
                    .w(px(120.))
                    .h(px(40.))
                    .child(div().w(px(10.)).h(px(10.)).bg(rgba(0xff0000ff))),
            )
            .child(
                div()
                    .id(ElementId::Name("dup".into()))
                    .w(px(120.))
                    .h(px(40.))
                    .child(div().w(px(10.)).h(px(10.)).bg(rgba(0x0000ffff))),
            )
    }
}

/// 截取窗口可比快照（场景图元 + 命中盒）。
fn capture(window: &Window) -> String {
    let scene = &window.rendered_frame.scene;
    format!(
        "quads={:?}\nhitboxes={:?}",
        scene.quads, window.rendered_frame.hitboxes
    )
}

/// 场景中所有图元的纵坐标（升序去重），用于定位子元素错位。
fn quad_origin_ys(window: &Window) -> Vec<f32> {
    let mut ys: Vec<f32> = window
        .rendered_frame
        .scene
        .quads
        .iter()
        .map(|quad| quad.bounds.origin.y.as_f32())
        .collect();
    ys.sort_by(f32::total_cmp);
    ys.dedup();
    ys
}

/// 两处同 id 的兄弟连绘三帧：不得 panic、场景稳定、子元素不错位、且复用真实发生。
#[test]
fn duplicated_element_id_siblings_keep_layout_across_frames() {
    let mut test_app = TestAppContext::single();
    let window_size = size(px(400.), px(300.));
    let window = test_app.open_window(window_size, |_, _| DuplicateRoot);
    let any = window.into();

    // 三个帧：首帧全量分配，后两帧都必须走保留复用（旧实现每帧重开临时节点）。
    let mut snapshots = Vec::new();
    for _ in 0..3 {
        let snapshot = test_app
            .update_window(any, |_, window, cx| {
                window.draw(cx).clear(cx);
                // 测试窗口的缩放因子不一定为 1，期望值按缩放折算。
                let expected = vec![0.0, 40.0 * window.scale_factor()];
                let stats = window.fast_stats.snapshot();
                (
                    capture(window),
                    quad_origin_ys(window),
                    expected,
                    stats.layout_nodes_reused,
                    stats.layout_nodes_allocated,
                )
            })
            .unwrap();
        snapshots.push(snapshot);
    }

    assert_eq!(snapshots[1].0, snapshots[0].0, "第二帧场景与首帧不一致");
    assert_eq!(snapshots[2].0, snapshots[0].0, "第三帧场景与首帧不一致");
    // 两处同 id 的兄弟各自的子元素仍停在自己父元素下方（0 与 40，而非都被当成根节点归零）。
    assert_eq!(snapshots[0].1, snapshots[0].2, "首帧子元素已错位");
    assert_eq!(snapshots[1].1, snapshots[1].2, "复用帧子元素错位");
    // 保留确实生效（根级无 id 元素的键按帧复位，否则永远命中不了）。
    assert!(
        snapshots[2].3 > 0,
        "静止帧没有复用任何布局节点，保留路径未生效"
    );
    // 静止帧不得再分配布局节点：旧实现把同键第二次请求做成「临时节点」，
    // 每帧都要重开两个再在帧末销毁，累计分配数会持续增长。
    assert_eq!(
        snapshots[2].4, snapshots[1].4,
        "静止帧仍在分配布局节点（同键重复请求未被保留）"
    );
}
