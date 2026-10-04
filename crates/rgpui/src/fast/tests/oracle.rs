//! 双窗对等 oracle：增量窗口与全量窗口逐帧场景一致（P2c）。
//!
//! 覆盖：静止帧、子视图 notify、被读实体无 notify 更新、滚动偏移、全局写入、
//! 列表 splice。任一步 divergent 即此测试失败并报出步骤。

use crate::{
    App, AppContext, BorrowAppContext, Context, Entity, Global, IntoElement, ListAlignment,
    ListState, ParentElement, Render, ScrollHandle, SharedString, Styled, TestAppContext, Window,
    div, list, point, px, size, v_flex,
};
use std::{cell::RefCell, rc::Rc};

/// oracle 标题全局量（根视图读取；写入即全部相关视图过期）。
struct OracleTitle(SharedString);

impl Global for OracleTitle {}

/// 被根视图读取的计数器实体（无 notify 更新也必须可见）。
struct Counter {
    value: usize,
}

/// 叶子视图（自身变更走 notify；见模块文档）。
struct OracleLeaf {
    text: SharedString,
}

impl Render for OracleLeaf {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(self.text.clone())
    }
}

/// 各窗口独立持有的句柄束（两窗操作历史完全一致）。
#[derive(Clone)]
struct OracleHandles {
    leaf_a: Entity<OracleLeaf>,
    leaf_b: Entity<OracleLeaf>,
    counter: Entity<Counter>,
    list: ListState,
    scroll: ScrollHandle,
}

/// oracle 根视图（读计数器实体 + 标题全局 + 嵌套两叶子 + 滚动区 + 列表）。
struct OracleRoot {
    handles: OracleHandles,
}

impl OracleRoot {
    /// 构造根视图与其子状态（子实体handle 供测试双窗同操作）。
    fn new(cx: &mut Context<Self>) -> Self {
        let leaf_a = cx.new(|_| OracleLeaf {
            text: SharedString::from("leaf-a"),
        });
        let leaf_b = cx.new(|_| OracleLeaf {
            text: SharedString::from("leaf-b"),
        });
        let counter = cx.new(|_| Counter { value: 0 });
        Self {
            handles: OracleHandles {
                leaf_a,
                leaf_b,
                counter,
                list: ListState::new(5, ListAlignment::Top, px(0.)),
                scroll: ScrollHandle::new(),
            },
        }
    }
}

impl Render for OracleRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.handles.counter.read(cx).value;
        let title = cx.global::<OracleTitle>().0.clone();
        // 把滚动版本纳入本视图依赖（模拟读取滚动状态的视图；P2 显式跟踪前哨）。
        cx.note_state_read(&self.handles.scroll.version());
        let list_state = self.handles.list.clone();
        v_flex()
            .child(div().child(SharedString::from(format!("{title} #{count}"))))
            .child(self.handles.leaf_a.clone())
            .child(self.handles.leaf_b.clone())
            .child(
                div()
                    .h(px(200.))
                    .overflow_hidden()
                    .children((0..8).map(|ix| {
                        div()
                            .h(px(30.))
                            .child(SharedString::from(format!("scroll-row {ix}")))
                    })),
            )
            .child(list(list_state, move |ix, _, _| {
                div()
                    .h(px(24.))
                    .child(SharedString::from(format!("list-row {ix}")))
                    .into_any_element()
            }))
    }
}

/// 截取窗口可比快照（场景图元 + 命中盒 + 调试边界）。
fn capture(window: &Window) -> String {
    let scene = &window.rendered_frame.scene;
    format!(
        "quads={:?}\nshadows={:?}\npaths={:?}\nunderlines={:?}\nmono={:?}\nsub={:?}\npoly={:?}\nsurfaces={:?}\nhitboxes={:?}\ndebug={:?}",
        scene.quads,
        scene.shadows,
        scene.paths,
        scene.underlines,
        scene.monochrome_sprites,
        scene.subpixel_sprites,
        scene.polychrome_sprites,
        scene.surfaces,
        window.rendered_frame.hitboxes,
        window.rendered_frame.debug_bounds_log,
    )
}

/// 双窗各绘制一帧并断言对等，返回 A 窗（复用／重建）计数。
fn draw_both(
    test_app: &mut TestAppContext,
    any_a: crate::AnyWindowHandle,
    any_b: crate::AnyWindowHandle,
    step: &str,
) -> (u64, u64) {
    let snap_a = test_app
        .update_window(any_a, |_, window, cx| {
            window.draw(cx).clear(cx);
            let stats = window.fast_stats.snapshot();
            (capture(window), stats.views_reused, stats.views_rebuilt)
        })
        .unwrap();
    let snap_b = test_app
        .update_window(any_b, |_, window, cx| {
            window.draw(cx).clear(cx);
            capture(window)
        })
        .unwrap();
    assert_eq!(snap_a.0, snap_b, "增量帧与全量帧在步骤 `{step}` divergent");
    (snap_a.1, snap_a.2)
}

/// 对双窗树执行同一操作。
fn drive_both(
    test_app: &mut TestAppContext,
    handles_a: &OracleHandles,
    handles_b: &OracleHandles,
    op: impl Fn(&OracleHandles, &mut App),
) {
    let handles_a = handles_a.clone();
    let handles_b = handles_b.clone();
    test_app.update(|cx| {
        op(&handles_a, cx);
        op(&handles_b, cx);
    });
}

/// 增量窗口与全量窗口在同一操作历史下逐帧一致，且增量窗口确实复用过。
#[test]
fn retained_matches_from_scratch() {
    let mut test_app = TestAppContext::single();
    test_app.update(|cx| {
        cx.set_global(OracleTitle(SharedString::from("oracle")));
    });

    let stash_a: Rc<RefCell<Option<OracleHandles>>> = Rc::default();
    let stash_b: Rc<RefCell<Option<OracleHandles>>> = Rc::default();
    let window_size = size(px(800.), px(600.));
    let win_a = test_app.open_window(window_size, {
        let stash = stash_a.clone();
        move |_, cx| {
            let root = OracleRoot::new(cx);
            *stash.borrow_mut() = Some(root.handles.clone());
            root
        }
    });
    let win_b = test_app.open_window(window_size, {
        let stash = stash_b.clone();
        move |_, cx| {
            let root = OracleRoot::new(cx);
            *stash.borrow_mut() = Some(root.handles.clone());
            root
        }
    });
    let any_a = win_a.into();
    let any_b = win_b.into();
    let (handles_a, handles_b) = (
        stash_a.borrow().clone().unwrap(),
        stash_b.borrow().clone().unwrap(),
    );

    // B 窗关闭保留：逐帧全量重建，作为真值。
    test_app
        .update_window(any_b, |_, window, _| {
            window.set_retention_override(Some(false));
        })
        .unwrap();

    // 基线 + 静止帧（A 应复用，见末尾断言）。
    let (reused_before, mut rebuilt_before) = draw_both(&mut test_app, any_a, any_b, "baseline");
    draw_both(&mut test_app, any_a, any_b, "static");
    draw_both(&mut test_app, any_a, any_b, "static-2");

    // 子视图 notify 更新：必须重建（脏传播）。
    drive_both(&mut test_app, &handles_a, &handles_b, |handles, cx| {
        handles.leaf_a.update(cx, |leaf, cx| {
            leaf.text = SharedString::from("leaf-a-notified");
            cx.notify();
        });
    });
    let (_, rebuilt) = draw_both(&mut test_app, any_a, any_b, "leaf-notify");
    assert!(rebuilt > rebuilt_before, "notify 后 A 窗未重建");
    rebuilt_before = rebuilt;

    // 被读实体无 notify 更新（必须可见）。
    drive_both(&mut test_app, &handles_a, &handles_b, |handles, cx| {
        handles.counter.update(cx, |counter, _| counter.value += 1);
    });
    let (_, rebuilt) = draw_both(&mut test_app, any_a, any_b, "counter-update");
    assert!(rebuilt > rebuilt_before, "无 notify 更新后 A 窗未重建");
    rebuilt_before = rebuilt;

    // 被读实体更新 + notify。
    drive_both(&mut test_app, &handles_a, &handles_b, |handles, cx| {
        handles.counter.update(cx, |counter, cx| {
            counter.value += 10;
            cx.notify();
        });
    });
    let (_, rebuilt) = draw_both(&mut test_app, any_a, any_b, "counter-notify");
    assert!(rebuilt > rebuilt_before, "notify 更新后 A 窗未重建");
    rebuilt_before = rebuilt;

    // 滚动偏移（无 notify，靠 StateVersion）。
    drive_both(&mut test_app, &handles_a, &handles_b, |handles, _| {
        handles.scroll.set_offset(point(px(0.), px(-40.)));
    });
    let (_, rebuilt) = draw_both(&mut test_app, any_a, any_b, "scroll");
    assert!(rebuilt > rebuilt_before, "滚动后 A 窗未重建");
    rebuilt_before = rebuilt;

    // 全局写入。
    drive_both(&mut test_app, &handles_a, &handles_b, |handles, cx| {
        let _ = handles;
        cx.update_global::<OracleTitle, _>(|title, _| {
            title.0 = SharedString::from("oracle-2");
        });
    });
    let (_, rebuilt) = draw_both(&mut test_app, any_a, any_b, "global");
    assert!(rebuilt > rebuilt_before, "全局写入后 A 窗未重建");
    rebuilt_before = rebuilt;

    // 列表 splice 新增行。
    drive_both(&mut test_app, &handles_a, &handles_b, |handles, _| {
        handles.list.splice(0..0, 2);
    });
    let (_, rebuilt) = draw_both(&mut test_app, any_a, any_b, "splice");
    assert!(rebuilt > rebuilt_before, "splice 后 A 窗未重建");

    // 收尾静止帧 + 非空洞断言：A 确实复用过。
    let (reused_after, _) = draw_both(&mut test_app, any_a, any_b, "static-end");
    assert!(
        reused_after > reused_before,
        "增量窗口从未复用，oracle 空洞通过"
    );
}
