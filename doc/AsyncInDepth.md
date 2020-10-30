## 深入异步(Async in depth)
到现在，我们完成了异步Rust和Tokio相当全面的介绍. 现在我们将更深入的研究Rust异步运行时模型. 在本教程最开始，我们暗示过Rust的异步采用了一种独特的方式. 现在我们将解释其含义.

## Futures
作为回顾，让我们写一个非常基础的异步函数. 与到目前为止的教程相比，这并不是什么新东西.

```rust
use tokio::net::TcpStream;

async fn my_async_fn() {
    println!("hello from async");
    let _socket = TcpStream::connect("127.0.0.1:3000").await.unwrap();
    println!("async TCP operation complete");
}
```

我们调用这个函数并得到一些返回值. 我们调用`.await`得到这个值.

```rust
#[tokio::main]
async fn main() {
    let what_is_this = my_async_fn();
    // 上面调用后，这里并没有任何打印内容
    what_is_this.await;
    // 文本被打印了，且socket链接建立和关闭
}
```

`my_async_fn()` 返回的是一个future值. 此Future它是一个实现了标准库中 [std::future::Future](https://doc.rust-lang.org/std/future/trait.Future.html) trait 的值. 它们是包含正在进行异步计算的值.

[std::future::Future](https://doc.rust-lang.org/std/future/trait.Future.html) trait定义如下:

```rust
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait Future {
    type Output;
    
    fn poll(self: Pin<&mut Self>, cx:&mut Context) -> Poll<Self::Output>;
}
```

[associated type](https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#specifying-placeholder-types-in-trait-definitions-with-associated-types) `Output` 是一个future完成后产生的类型. [Pin](https://doc.rust-lang.org/std/pin/index.html) 类型是Rust在 `async` 函数中如何支持借用. 查看 [standard library](https://doc.rust-lang.org/std/pin/index.html) 了解更多的细节. 与其它语言实现的future不一样， 一个Rust的future不代表在后台发生的计算，而是Rust的future就是计算本身. future的所有者通过轮询future来推进计算. 这是通过调用 `Future::poll`来完成的.

### 实现 `Future` (Implementing `Future`)

让我们来实现一个非常简单的future. 它有以下几个特点:

1. 等待到一个特定的时间点.
2. 输出一些文本到STDOUT.
3. 产生一个String.

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

struct Delay {
    when: Instant,
}

impl Future for Delay {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<&'static str>
    {
        if Instant::now() >= self.when {
            println!("Hello world");
            Poll::Ready("done")
        } else {
            // 现在忽略这一行
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[tokio::main]
async fn main() {
    let when = Instant::now() + Duration::from_millis(10);
    let future = Delay { when };

    let out = future.await;
    assert_eq!(out, "done");
}
```

### Async fn as a Future

在main函数中，我们实例化一个future并在它上面调用 `.await`. 在异步函数中，我们可以在任何实现了 `Future` 的值上调用 `.await` .  反过来说， 调用一个 `async` 函数会返回一个实现了 `Future` 的匿名类型. 在 `async fn main()` 中，生成的future大致为:

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

enum MainFuture {
    // 初始化时，从未轮询过
    State0,
    // 等待 `延迟` ， 比如. `future.await` 这一行.
    State1(Delay),
    // future已经完成.
    Terminated,
}

impl Future for MainFuture {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<()>
    {
        use MainFuture::*;

        loop {
            match *self {
                State0 => {
                    let when = Instant::now() +
                        Duration::from_millis(10);
                    let future = Delay { when };
                    *self = State1(future);
                }
                State1(ref mut my_future) => {
                    match Pin::new(my_future).poll(cx) {
                        Poll::Ready(out) => {
                            assert_eq!(out， "done");
                            *self = Terminated;
                            return Poll::Ready(());
                        }
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                    }
                }
                Terminated => {
                    panic!("future polled after completion")
                }
            }
        }
    }
}
```

Rust的Future是一种**状态机**. 这里 `MainFuture` 代表future可能的状态枚举. future开始于`State0` 状态. 当调用`poll`时， future会尝试尽可能的推进其内部的状态.如果future能够完成，则返回包含异步计算输出的`Poll::Ready`.

如果future**不能够**完成， 通常是由于资源不够而等待，这个时候返回`Poll::Pending`. 接收到`Poll::Pending`会向调用者表明future会在将来某个时刻完成，并且调用者应该稍候再次调用`poll`函数.

我们还看到future由其它future组合. 在外部future上调用`poll`会导致在内部future上调用`poll`函数.

## 执行器(Executors)

异步Rust函数返回future. 必须在Future上调用`poll`来推进其状态. Future可以被其它Future组合. 因此，问题来了，调用最外部的future的`poll`意味着什么?

回想一下，要运行异步函数，必须将它们传递给`tokio::spawn`或者使用`#[tokio::main]`注解main函数. 这样的结果是生成的外部future提交给Tokio的执行器.执行器负责在外部Future上调用`Future::poll`，来驱动异步计算的完成.

### Mini Tokio

为了更好的理解这一切是如何融合的，让我们实现自己的迷你版本的Tokio！完整的代码在 [这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/mini-tokio/src/main.rs) .

```rust
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use futures::task;

fn main() {
    let mut mini_tokio = MiniTokio::new();

    mini_tokio.spawn(async {
        let when = Instant::now() + Duration::from_millis(10);
        let future = Delay { when };

        let out = future.await;
        assert_eq!(out, "done");
    });

    mini_tokio.run();
}

struct MiniTokio {
    tasks: VecDeque<Task>,
}

type Task = Pin<Box<dyn Future<Output = ()> + Send>>;

impl MiniTokio {
    fn new() -> MiniTokio {
        MiniTokio {
            tasks: VecDeque::new(),
        }
    }
    
    /// 在 mini-tokio 实例之上产生一个future
    fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.push_back(Box::pin(future));
    }
    
    fn run(&mut self) {
        let waker = task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        
        while let Some(mut task) = self.tasks.pop_front() {
            if task.as_mut().poll(&mut cx).is_pending() {
                self.tasks.push_back(task);
            }
        }
    }
}
```

这将运行异步块. 使用请求延迟来创建一个`Delay`实例并等待它. 然而，我们的实现到目前为止有一个重大的缺陷. 我们的执行器绝不会休眠. 执行器不断
循环所有产生的future并对其进行轮询. 大多时候，future不准备执行更多的工作，并会返回`Poll::pending`. 这一过程会消耗CPU并且通常没有效率.

理想的情况下，我们仅仅想让mini-tokio在future在有进展的时候去轮询future. 当阻塞任务的资源准备好执行请求操作的时候，就会发生这种情况. 如果
任务想从TCP socket中读取数据，那么我们只想在TCP socket接收到数据时轮询任务. 在我们的方案中，任务在到达指定的`Instant`时被阻塞. 理想情况下
mini-tokio只会在该时间过去后再轮询任务.

为了达到这一目的，在对一个资源进行轮询且资源未准备好时，资源转换为就绪状态后将发送一个通知.

## 唤醒(Wakers)
通过该系统(译者注: 指唤醒)，资源在通知等待它的任务时表明已经准备就绪，可以继续进行一些其它的操作了.

让我们再次看看`Future::poll`的定义:

```rust
fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
```

`poll`函数中的`Context`参数有一个`waker()`方法. 此方法返回一个绑定到当前任务的[Waker](https://doc.rust-lang.org/std/task/struct.Waker.html) .
`Waker`中又有一个`wake()`方法. 调用这个方法会向执行器发出信息，说明应该安排相关任务的执行计划. 当资源的状态转换到就绪状态时，它们会调用`wake()`方法，来通知执行者轮询任务来推进资源的状态.

### 更新`Delay`(Updating `Delay`)
我们能更新`Delay`来使用唤醒(wakers):

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::thread;

struct Delay {
    when: Instant,
}

impl Future for Delay {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<&'static str>
    {
        if Instant::now() >= self.when {
            println!("Hello world");
            Poll::Ready("done")
        } else {
            // 在当前任务上获取一个waker句柄
            let waker = cx.waker().clone();
            let when = self.when;

            // 生产一个定时器线程
            thread::spawn(move || {
                let now = Instant::now();

                if now < when {
                    thread::sleep(when - now);
                }

                waker.wake();
            });

            Poll::Pending
        }
    }
}
```

现在，一旦请求的持续时间过去后，调用任务就会得到通知，执行器可以确保再次安排任务. 下一步就是更新mini-tokio来监听唤醒通知.

我们的`Delay`实现仍然有一些其它的问题. 我们将在后面修复它.

```text
当一个future返回 Poll::Pending时，它必须确保waker能在某个时刻发出信息. 忘记此操作的结果就是任务会无限的挂起.
返回 Poll::Pending后忘记唤醒任务是常见bug的来源.
```

回忆一下`Delay`的第一个迭代版本. 下面是future的实现:

```rust
impl Future for Delay {
    type Output = &'static str;
    
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<&'static str> {
        if Instant::now() >= self.when {
            println!("Hello world");
            Poll::Ready("done")
        }else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
```

在返回`Poll::Pending`之前，我们调用了`cx.waker().wake_by_ref()`. 这是为了满足future的组合操作. 通过返回`Poll::Pending`，我们负责
发信号给waker. 因为我们没有实现计时器(timer)线程，所以我们向唤醒程序(waker)发送了内联信息. 这样做的结果是为了future能重新被调度，再次执行，
并且可能还没有完全准备好.

请注意，你可以向waker发送更多的不是必须的信号. 在这种特殊的情况下，即使我们没有准备好继续操作，我们也会向waker发出信号. 除了浪费一个CPU时钟周期外，并没有什么问题. 但是，这种特殊的实现将导致非常繁忙的循环.

### 更新Mini-Tokio(Updating Mini-Tokio)
接下来就是更新Mini Tokio 来接收waker的通知. 我们想让执行器在仅当任务被唤醒时才运行它们，为了做到这一点，Mini Tokio将提供自己的waker.
当waker被调用时，与它相关的任务会被排队执行. Mini Tokio在轮询future时将waker传递给future.

更新后的Mini Tokio将使用channel来存储计划任务. 通道(Channel)允许任务以队列的方式从任意线程执行. Wakers必须是 `Send`和`Sync` 类型的，
因此我们使用 crossbeam 包中的channel，因为标准库中的channel不是`Sync`的.

`Send` 与 `Sync` 是Rust提供的与并发相关的一种标记trait. Send 类型可以在不同的线程中传递. 大多数类型都是 Send ，但是像 [Rc](https://doc.rust-lang.org/std/rc/struct.Rc.html) 却不是. 可以通过不可变引用并发访问的类型是 Sync. 一个类型是 Send 但不是 Sync ---- 一个很好的例子是 [Cell](https://doc.rust-lang.org/std/cell/struct.Cell.html)，可以通过不可变引用对其修改，因此它不能并发的共享访问.

更多关于 `Send` 与 `Sync` 的细节可以参考[chapter in the Rust book](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html).

在 `Cargo.toml` 中添加如下依赖:

```toml
crossbeam = "0.7"
```

然后，更新`MiniTokio` 结构体:

```rust
use crossbeam::channel;
use std::sync::Arc;

struct MiniTokio {
    scheduled: channel::Receiver<Arc<Task>>,
    sender: channel::Sender<Arc<Task>>,
}

struct Task {
    // 这一块后面再填写
}
```

Wakers是`Sync`类型的并且它可以被克隆(Clone). 当调用`wake`时，必须安排任务来执行. 为了实现这个，我们使用channel. 当在waker上调用`wake()`时，
任务被推送到channel的发送方. 我们的`Task`结构体将实现wake的逻辑. 为了做到这一点，它需要包含生成的Future和channel发送方.

```rust
use std::sync::{Arc, Mutex};

struct Task {
    // Mutex能使用任务实现'同步(sync)'效果，在任意时刻仅能有一个线程能够访问future.
    // Mutex (在此场景下)不需要非常正确，真实的tokio没有在这里使用Mutex，但是真实的tokio
    // 使用了更多行的代码来实现这一点.
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    executor: channel::Sender<Arc<Task>>,
}

impl Task {
    fn schedule(self: &Arc<Self>) {
        self.executor.send(self.clone());
    }
}
```

为了安排任务，`Arc`将会被clone，并将它通过channel发送. 现在，我们需要将`schedule`函数与 [std::task::Waker](https://doc.rust-lang.org/std/task/struct.Waker.html) 挂钩.
标准库提供了一套低级别的API [manual vtable construction](https://doc.rust-lang.org/std/task/struct.RawWakerVTable.html) 来做这个. 这种策略为实现者提供了最大的灵活性，
但是需要大量的unsafe(不安全)的样板代码. 取而代之的是，我们可以直接使用[RawWakerVTable](https://doc.rust-lang.org/std/task/struct.RawWakerVTable.html)，
我们使用[futures](https://docs.rs/futures/)包提供的[ArcWake](https://docs.rs/futures/0.3/futures/task/trait.ArcWake.html) 工具.
这可以使我们能实现一个简单的trait，来将我们的`Task`结构暴露为一个waker.

在`Cargo.toml`中添加如下依赖来拉取`futures`.

```toml
futures = "0.3"
```

然后实现[futures::task::ArcWake](https://docs.rs/futures/0.3/futures/task/trait.ArcWake.html) .

```rust
use futures::task::ArcWake;
use std::sync::Arc;

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.schedule();
    }
}
```

当上面的计时器(timer)线程调用`waker.wake()`时，任务被发送到channel通道中去. 下一步我们在`MiniTokio::run()`函数中实现接收和执行任务的功能.

```rust
impl MiniTokio {
    fn run(&self) {
        while let Ok(task) = self.scheduled.recv() {
            task.poll();
        }
    }

    /// 初始化一个新的 mini-tokio 实例.
    fn new() -> MiniTokio {
        let (sender, scheduled) = channel::unbounded();

        MiniTokio { scheduled, sender }
    }

    /// 在 mini-tokio 实例上产生一个 future
    ///
    /// 给future包装task并推其推送到 `scheduled` 队列中.
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Task::spawn(future, &self.sender);
    }
}

impl Task {
    fn poll(self: Arc<Self>) {
        // 从task实例上创建一个waker. 它使用 ArcWake
        let waker = task::waker(self.clone());
        let mut cx = Context::from_waker(&waker);

        // 没有其它线程试图锁住future
        let mut future = self.future.try_lock().unwrap();

        // 轮询future
        let _ = future.as_mut().poll(&mut cx);
    }

    // 使用指定的future产生一个新的task.
    //
    // 初始化一个新的包含了指定future的task，并将其它推送给 sender. channel另外一半的receiver将接收到它并执行.
    fn spawn<F>(future: F, sender: &channel::Sender<Arc<Task>>)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = Arc::new(Task {
            future: Mutex::new(Box::pin(future)),
            executor: sender.clone(),
        });

        let _ = sender.send(task);
    }

}
```

这里发生了多件事. 首先，实现了`MiniTokio::run()`. 这个函数循环运行，从通道中接收计划任务. 当任务被唤醒时，任务被推送到channel中，这些
任务在执行时能够取得进展(译者注: 指poll后任务本身的状态能得到推进).

另外，`MiniTokio::new()`与`MiniTokio::spawn()`函数也使用channel来调整了一下，而不是使用`VecDeque`. 当一个新的任务产生时，为它们分配
channel 发送部分的副本，任务可以在运行时使用该副本来调度本身.

`Task::poll()` 函数使用来自`futures`包中的`ArcWake`工具创建一个waker. 此waker用来创建一个`task::Context`. `task::Context`传递给`poll`.

## 概要(Summary)
我们现在已经看到了异步Rust的端到端原理示例. Rust的`async/await` 特性背后由trait支持. 这就允许使用第三方包，像tokio来提供执行细节.

* Rust的异步操作是惰性的，需要调用者对其进行轮询.
* Wakers被传递给future,以将future与调用它的任务联系起来.
* 当一个资源没有准备好完成时，`Poll::Pending`被返回并记录任务的唤醒程序(waker).
* 当一个资源变为就绪状态时，就会通知任务的唤醒程序(waker).
* 执行器接收到通知并安排任务来执行.
* 任务再一次被轮询，这一次资源是就绪状态并且任务能够取得进展.

## 一些零碎的结论(A few loose ends)
回顾一下，当我们实现`Delay`时，我们说过还要更多的问题要修复. Rust的异步模型允许单个future在执行时跨任务移动. 考虑一下如下代码:

```rust
use futures::future::poll_fn;
use std::future::Future;
use std::pin::Pin;

#[tokio::main]
async fn main() {
    let when = Instant::now() + Duration::from_millis(10);
    let mut delay = Some(Delay { when });

    poll_fn(move |cx| {
        let mut delay = delay.take().unwrap();
        let res = Pin::new(&mut delay).poll(cx);
        assert!(res.is_pending());
        tokio::spawn(async move {
            delay.await;
        });

        Poll::Ready(())
    }).await;
}
```

`poll_fn` 函数使用闭包来创建一个`Future`实例. 上面的代码片段创建了一个`Delay`实例，并将其轮询一次，然后将`Delay`实例发送给一个新的任务，
再等待它. 在这个示例中，使用不周的`Waker`实例多次调用`Delay::poll`. 我们早期的实现中无法处理这种情况，并且由于通知了错误的任务，因此产生的
任务会永远处于休眠状态.

为了修复我们早期的实现，我们可以像下面这样做:

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

struct Delay {
    when: Instant,
    waker: Option<Arc<Mutex<Waker>>>,
}

impl Future for Delay {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        // 首时，如果第一次future被调用，产生一个计时器线程
        // 如果计时器线程已经在运行了，要确保存储的 waker能匹配上当前任务的 waker
        if let Some(waker) = &self.waker {
            let mut waker = waker.lock().unwrap();

            // 检查存储的waker能否匹配上当前任务的waker
            // 这是必须的，因为 Delay future实例可能在调用poll时移动在不同的task中去
            // 如果发生了这种情况, 给定的“Context”中包含的waker将有所不同，我们必须要更新存储的waker以反映此更改.
            if !waker.will_wake(cx.waker()) {
                *waker = cx.waker().clone();
            }
        } else {
            let when = self.when;
            let waker = Arc::new(Mutex::new(cx.waker().clone()));
            self.waker = Some(waker.clone());

            // 这是第一次 poll 被调用时产生一个计时器线程
            thread::spawn(move || {
                let now = Instant::now();

                if now < when {
                    thread::sleep(when - now);
                }

                // 持续时间过去后，通过激活waker来通知调用者.
                let waker = waker.lock().unwrap();
                waker.wake_by_ref();
            });
        }

        // 一旦waker被存储且计时器已经开始，就是检查delay是否完成的时候了.
        // 通过检查当前时刻来完成. 
        // 
        // 如果持续时间过了后, future已经完成 Poll::Ready就会返回
        if Instant::now() >= self.when {
            Poll::Ready(())
        } else {
            // 持续时间没有过去，future没有完成就返回 PollPending
            //
            // Future trait 要求当返回 Pending 时，future将确保一旦再次对future进行轮询，就会发出指定唤醒信息.
            // 
            // 在我们的例子中，通过这里返回的 Pending 我们可以保证一旦请求的持续时间过去后，我们将调用包含在 Context 参数中的指定waker
            // 我们通过产生一个计时器线程来确保这一点.
            //
            // 如果我们忘记激活waker，任务将会无限的持起.
            Poll::Pending
        }
    }
}
```

它涉及到一点，但是这个想法是，在每次轮询时，future都会检查所提供的waker是否与先前记录的waker相匹配. 如果两个waker匹配，则什么也不发生.
如果它们不匹配，则原来记录的waker必须被更新.

### `Notify` **utility**
我们演示了如何使用waker手动实现`Delay` future. Wakers是异步Rust能工作的基础. 通常，不需要降低到该级别. 比如说，在`Delay`的案例中，
我们可以使用[tokio::sync::Notify](https://docs.rs/tokio/0.3/tokio/sync/struct.Notify.html) 工具完全使用`async/await`来实现它.
这个实用工具提供了基础的任务通知机制. 它处理了waker的一些细节，包括确保记录的waker与当前任务的waker匹配.

使用[Notify](https://docs.rs/tokio/0.3/tokio/sync/struct.Notify.html)，我们可以像下面这样，使用 `async/await` 实现`Delay`功能:
```rust
use tokio::sync::Notify;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;

async fn delay(dur: Duration) {
    let when = Instant::now() + dur;
    let notify = Arc::new(Notify::new());
    let notify2 = notify.clone();

    thread::spawn(move || {
        let now = Instant::now();

        if now < when {
            thread::sleep(when - now);
        }

        notify2.notify_one();
    });


    notify.notified().await;
}
```


&larr; [Framing](Framing.md)

&rarr; [Select](Select.md)
