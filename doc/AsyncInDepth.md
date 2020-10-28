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

