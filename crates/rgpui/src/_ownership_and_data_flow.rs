//! 在 RGPUI 中，应用中的每个模型或视图实际上都由一个名为 `App` 的顶层对象所拥有。当创建一个新的 Entity 或视图时（统称为 _entities_），应用会接管它们的状态所有权，使它们能够参与各种应用服务并与其他 entities 交互。
//!
//! 为了说明，考虑下面这个简单的应用。我们通过调用 `run` 并传入一个回调来启动应用，该回调接收一个指向拥有应用所有状态的 `App` 的引用。这个 `App` 是我们访问所有应用级服务的入口，例如打开窗口、显示对话框等。它还有一个 `insert_entity` 方法，下面通过调用它来创建一个 entity 并将其所有权交给应用。
//!
//! ```no_run
//! # use rgpui::{App, AppContext, Application, Entity};
//! # struct Counter {
//! #     count: usize,
//! # }
//! rgpui_platform::application().run(|cx: &mut App| {
//!     let _counter: Entity<Counter> = cx.new(|_cx| Counter { count: 0 });
//!     // ...
//! });
//! ```
//!
//! `new_entity` 的调用返回一个 _entity 句柄_，它携带一个基于所引用对象类型的类型参数。这个 `Entity<Counter>` 句柄本身并不提供对 entity 状态的访问。它仅仅是一个惰性标识符加上编译期类型标签，并持有一个指向底层 `Counter` 对象的引用计数指针，该对象由应用所拥有。
//!
//! 就像 Rust 标准库中的 `Rc` 一样，当句柄被克隆时引用计数递增，当句柄被丢弃时引用计数递减，从而实现对底层模型的共享所有权。但与 `Rc` 不同的是，它只有在 `App` 引用可用时才提供对模型状态的访问。句柄并不真正"拥有"状态，但它可以用来从其真正的拥有者——`App`——访问状态。为简洁起见，省略部分初始化代码：
//!
//! ```no_run
//! # use rgpui::{App, AppContext, Application, Context, Entity};
//! # struct Counter {
//! #     count: usize,
//! # }
//! rgpui_platform::application().run(|cx: &mut App| {
//!     let counter: Entity<Counter> = cx.new(|_cx| Counter { count: 0 });
//!     // 调用 `update` 以访问模型的状态。
//!     counter.update(cx, |counter: &mut Counter, _cx: &mut Context<Counter>| {
//!         counter.count += 1;
//!     });
//! });
//! ```
//!
//! 为了更新计数器，我们在句柄上调用 `update`，传入 context 引用和一个回调。回调会接收一个指向计数器的可变引用，可用于操纵状态。
//!
//! 回调还会接收第二个 `Context<Counter>` 引用。这个引用类似于传给 `run` 回调的 `App` 引用。`Context` 实际上是 `App` 的包装器，包含一些额外数据以指示它与哪个特定 entity 绑定；本例中是计数器。
//!
//! 除了 `App` 提供的应用级服务外，`Context` 还提供了 entity 级服务的访问。例如，可以用来通知该 entity 的观察者其状态已更改。我们在示例中通过调用 `cx.notify()` 来实现这一点。
//!
//! ```no_run
//! # use rgpui::{App, AppContext, Application, Entity};
//! # struct Counter {
//! #     count: usize,
//! # }
//! rgpui_platform::application().run(|cx: &mut App| {
//!     let counter: Entity<Counter> = cx.new(|_cx| Counter { count: 0 });
//!     counter.update(cx, |counter, cx| {
//!         counter.count += 1;
//!         cx.notify(); // 通知观察者
//!     });
//! });
//! ```
//!
//! 接下来，这些通知需要被观察和响应。在更新计数器之前，我们将构建第二个计数器来观察它。每当第一个计数器发生变化，其计数的两倍就会赋值给第二个计数器。注意 `observe` 是在第二个计数器所属的 `Context` 上调用的，以便在第一个计数器发出通知时使其也收到通知。`observe` 的调用返回一个 `Subscription`，通过 `detach` 来保持此行为，只要两个计数器都存在。我们也可以存储这个 subscription 并在需要时丢弃它来取消此行为。
//!
//! `observe` 回调接收一个指向观察者的可变引用和一个指向被观察计数器的 _句柄_，我们通过 `read` 方法访问其状态。
//!
//! ```no_run
//!  # use rgpui::{App, AppContext, Application, Entity, prelude::*};
//!  # struct Counter {
//!  #     count: usize,
//!  # }
//!  rgpui_platform::application().run(|cx: &mut App| {
//!      let first_counter: Entity<Counter> = cx.new(|_cx| Counter { count: 0 });
//!
//!      let second_counter = cx.new(|cx: &mut Context<Counter>| {
//!          // 注意我们可以在 Counter 甚至还没创建时就设置好回调！
//!          cx.observe(
//!              &first_counter,
//!              |second: &mut Counter, first: Entity<Counter>, cx| {
//!                  second.count = first.read(cx).count * 2;
//!              },
//!          )
//!          .detach();
//!
//!          Counter { count: 0 }
//!      });
//!
//!      first_counter.update(cx, |counter, cx| {
//!          counter.count += 1;
//!          cx.notify();
//!      });
//!
//!      assert_eq!(second_counter.read(cx).count, 2);
//!  });
//! ```
//!
//! 更新第一个计数器后，可以观察到观察者的状态按照我们的 subscription 保持同步。
//!
//! 除了用于指示 entity 状态已更改的 `observe` 和 `notify` 之外，RGPUI 还提供了 `subscribe` 和 `emit`，使 entity 能够发出带类型的事件。要加入此系统，发出事件的对象必须实现 `EventEmitter` trait。
//!
//! 让我们引入一个新的事件类型 `CounterChangeEvent`，然后声明 `Counter` 可以发出此类型的事件：
//!
//! ```no_run
//! use rgpui::EventEmitter;
//! # struct Counter {
//! #     count: usize,
//! # }
//! struct CounterChangeEvent {
//!     increment: usize,
//! }
//!
//! impl EventEmitter<CounterChangeEvent> for Counter {}
//! ```
//!
//! 接下来，应更新示例，将观察替换为订阅。每当计数器递增时，就会发出一个 `Change` 事件来指示增长的幅度。
//!
//! ```no_run
//! # use rgpui::{App, AppContext, Application, Context, Entity, EventEmitter};
//! # struct Counter {
//! #     count: usize,
//! # }
//! # struct CounterChangeEvent {
//! #     increment: usize,
//! # }
//! # impl EventEmitter<CounterChangeEvent> for Counter {}
//! rgpui_platform::application().run(|cx: &mut App| {
//!     let first_counter: Entity<Counter> = cx.new(|_cx| Counter { count: 0 });
//!
//!     let second_counter = cx.new(|cx: &mut Context<Counter>| {
//!         // 注意我们可以在 Counter 甚至还没创建时就设置好回调！
//!         cx.subscribe(&first_counter, |second: &mut Counter, _first: Entity<Counter>, event, _cx| {
//!             second.count += event.increment * 2;
//!         })
//!         .detach();
//!
//!         Counter {
//!             count: first_counter.read(cx).count * 2,
//!         }
//!     });
//!
//!     first_counter.update(cx, |first, cx| {
//!         first.count += 2;
//!         cx.emit(CounterChangeEvent { increment: 2 });
//!         cx.notify();
//!     });
//!
//!     assert_eq!(second_counter.read(cx).count, 4);
//! });
//! ```
