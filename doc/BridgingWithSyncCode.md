## 使用同步代码桥接(Bridging with sync code)
在大多数使用 `Tokio` 的示例中，我们使用 `#[tokio::main]` 来标记 `main` 函数，并使得整个工程是异步的。
然而并不是所有项目都需要这样。比方说，GUI类的应用可能希望在main线程上运行GUI代码，在另外一个线程上运行`tokio`
的运行时。

这一页将告诉你如何使用 `async/await` 来隔离项目中的一小部分。

### `#[tokio::main]` 指什么? (Waht `#[tokio::main]` expands to)
`#[tokio::main]` 是一个宏，是一个用来替代调用非异步代码 `main` 函数的宏，并启动一个运行时。(有点绕，自行组织了下好理解点)。比如像下面这样:

```rust
#[tokio::main]
async fn main() {
    println!("hello world!");
}
```

它可以转换成如下写法:

```rust
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            println!("Hello world");
        })
}
```

通过宏，为了在我们自己的项目中使用 `async/await` , 我们可以做一些类似的事，利用 `block_on` 方法在适当的地方进入异步上下文。

### mini-redis的同步接口(A synchronous interface to mini-redis) 

在这一章节中，我们将介绍如何通过存储 `Runtime` 对象并使用它的 `block_on` 方法来构建与 `mini-redis` 同步的接口。在下面的章节中我们将讨论
一些替代方法和如何使用这些替代方法。

我们要包装的接口是一个异步的 [Client](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html) 类型。它有几方法，我们将
实现这几个方法的阻塞版本:

* [Client::get](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.get)
* [Client::set](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.set)
* [Client::set_expires](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.set_expires)
* [Client::publish](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.publish)
* [Client::subscribe](https://docs.rs/mini-redis/0.4/mini_redis/client/struct.Client.html#method.subscribe)

为了做到这一点，我们引入一个 `src/blocking_client.rs` 文件，并使用异步 `Client` 类型的包装结构对其进行初始化。

```rust
use tokio::net::ToSocketAddrs;
use tokio::runtime::Runtime;

pub use crate::client::Message;

/// 与Redis server 建立链接
pub struct BlockingClient {
    /// The asynchronous `Client`.
    inner: crate::client::Client,

    /// A `current_thread` runtime for executing operations on the
    /// asynchronous client in a blocking manner.
    /// 一个 `current_thread` 运行时用来在异步 client 上执行阻塞操作
    rt: Runtime,
}

pub fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<BlockingClient> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    // Call the asynchronous connect method using the runtime.
    // 使用rt(实际上是tokio的异步运行时)来进行异步connect
    let inner = rt.block_on(crate::client::connect(addr))?;

    Ok(BlockingClient { inner, rt })
}
```

这里，我们在构造函数中展示了如何在非异步上下文中执行异步方法的示例。我们在 `tokio` 的异步运行时 [Runtime](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html)
类型上使用 [block_on](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.block_on) 方法，它执行一个异步方法并返回结果。


有一个很重要的细节是这里使用了 [current_thread](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.new_current_thread) 运行时。
通常，当我们使用 Tokio 时默认情况下使用使用 [multi_thread](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.new_multi_thread) 运行时，
它会产生一堆的后台线程因此它可以在同一时刻非常高效的运行很多东西。在我们的示例中，我们在同一时候会仅仅只做一件事，因此我们没必要在后台运行多个线程。这使得
[current_thread](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.new_current_thread) 非常适合，而不是产生多个线程。

调用 [enable_all](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.enable_all) 在tokio的运行时上启动IO和计时驱动。
如果它没被启动，运行时将不能执行IO或计时器功能。

```text
因为 `current_thread` 运行时不产生线程，它仅仅在 `block_on` 被调用时运行。一旦 `block_on` 返回，所有在运行时上产生的任务都会被冻结，直到你
再次调用 `block_on` 方法。 如果在不调用 `block_on` 时产生的任务也要保持运行，那么请使用 `multi_thread` 运行时。
```

一旦我们有了这样的结构，大部分方法实现起来就很容易了:

```rust
use bytes::Bytes;
use std::time::Duration;

impl BlockingClient {
    pub fn get(&mut self, key: &str) -> crate::Result<Option<Bytes>> {
        self.rt.block_on(self.inner.get(key))
    }

    pub fn set(&mut self, key: &str, value: Bytes) -> crate::Result<()> {
        self.rt.block_on(self.inner.set(key, value))
    }

    pub fn set_expires(
        &mut self,
        key: &str,
        value: Bytes,
        expiration: Duration,
    ) -> crate::Result<()> {
        self.rt.block_on(self.inner.set_expires(key, value, expiration))
    }

    pub fn publish(&mut self, channel: &str, message: Bytes) -> crate::Result<u64> {
        self.rt.block_on(self.inner.publish(channel, message))
    }
}
```

[Client::subscribe] 方法更加有趣，因为它将 `Client` 对象转换成 `Subscriber` 对象。 我们可以像下面这样实现它: 

```rust
/// 一个能进入的 发布/订阅模式的客户端
///
/// Once clients subscribe to a channel, they may only perform
/// 一旦有客户端订阅一个通道，它们仅能执行 pub/sub 相关的命令。 
/// pub/sub related commands. The `BlockingClient` type is
/// `BlockingClient` 类型被转换成一个 `BlockingSubscriber` 类型是为了防止非 pub/sub 方法被调用。
/// transitioned to a `BlockingSubscriber` type in order to
/// prevent non-pub/sub methods from being called.
pub struct BlockingSubscriber {
    /// The asynchronous `Subscriber`.
    /// 异步 `Subscriber`
    inner: crate::client::Subscriber,

    /// A `current_thread` runtime for executing operations on the
    /// asynchronous client in a blocking manner.
    /// `current_thread` 运行时用于以阻塞的方式来运行异步client.
    rt: Runtime,
}

impl BlockingClient {
    pub fn subscribe(self, channels: Vec<String>) -> crate::Result<BlockingSubscriber> {
        let subscriber = self.rt.block_on(self.inner.subscribe(channels))?;
        Ok(BlockingSubscriber {
            inner: subscriber,
            rt: self.rt,
        })
    }
}

impl BlockingSubscriber {
    pub fn get_subscribed(&self) -> &[String] {
        self.inner.get_subscribed()
    }

    pub fn next_message(&mut self) -> crate::Result<Option<Message>> {
        self.rt.block_on(self.inner.next_message())
    }

    pub fn subscribe(&mut self, channels: &[String]) -> crate::Result<()> {
        self.rt.block_on(self.inner.subscribe(channels))
    }

    pub fn unsubscribe(&mut self, channels: &[String]) -> crate::Result<()> {
        self.rt.block_on(self.inner.unsubscribe(channels))
    }
}
```

因此，`subscribe` 方法会首先使用运行时将异步的 `Client` 转换成异步的 `Subscriber` 。 然后它将生成的 `Subscriber`与 `Runtime` 一起存储
，并使用 [block_on](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.block_on) 来实现各种方法。

注意到，异步的 `Subscriber` 结构体有一个非异步的方法 `get_subscribed` 。为了处理这个，我们直接使用非运行时的方式来调用它。

### 其它方法(Other approaches)
上面的章节解释了实现同步包装器的简单方式，但这不是唯一的方法。一般的方法有:

* 创建一个 [Runtime](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html) 并在异步代码上调用 [block_on](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.block_on) 
* 创建一个 [Runtime](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html) 并在它上面 [Spawn](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.spawn) 一些事。
* 在一个分隔的线程上运行一个 `Runtime` 并给它发消息。

我们已经看到了第一种的实现方式，另外两种将在下面来介绍。


#### 在一个Runtime上产生一个东西 (Spawning things on a runtime)
[Runtime](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html) 对象上有一个 [spawn](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.spawn) 方法。
当我们调用这个方法时，你可以在运行时上产生一个新任务。比如像下面这样:

```rust
use tokio::runtime::Builder;
use tokio::time::{sleep, Duration};

fn main() {
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(10);
    for i in 0..10 {
        handles.push(runtime.spawn(my_bg_task(i)));
    }

    // Do something time-consuming while the background tasks execute.
    // 当后台任务执行时做一些事来消耗下时间
    std::thread::sleep(Duration::from_millis(750));
    println!("Finished time-consuming task.");

    // Wait for all of them to complete.
    // 等待所有任务完成
    for handle in handles {
        // The `spawn` method returns a `JoinHandle`. A `JoinHandle` is
        // a future, so we can wait for it using `block_on`.
        
        // spawn 方法返回一个 JoinHandle. 它是一个 future, 因此我们可以在它上面使用 block_on 
        runtime.block_on(handle).unwrap();
    }
}

async fn my_bg_task(i: u64) {
    // By subtracting, the tasks with larger values of i sleep for a
    // shorter duration.
    
    // 通过相减，较大值的任务 sleep 时间更短
    let millis = 1000 - 50 * i;
    println!("Task {} sleeping for {} ms.", i, millis);

    sleep(Duration::from_millis(millis)).await;

    println!("Task {} stopping.", i);
}
```

```text
Task 0 sleeping for 1000 ms.
Task 1 sleeping for 950 ms.
Task 2 sleeping for 900 ms.
Task 3 sleeping for 850 ms.
Task 4 sleeping for 800 ms.
Task 5 sleeping for 750 ms.
Task 6 sleeping for 700 ms.
Task 7 sleeping for 650 ms.
Task 8 sleeping for 600 ms.
Task 9 sleeping for 550 ms.
Task 9 stopping.
Task 8 stopping.
Task 7 stopping.
Task 6 stopping.
Finished time-consuming task.
Task 5 stopping.
Task 4 stopping.
Task 3 stopping.
Task 2 stopping.
Task 1 stopping.
Task 0 stopping.
```

在上面的示例中，我们在运行时上产生了10个后台任务，并等待所有任务完成。比如，这可能是在图形应用程序中实现后台联网的好方法，因为网络请求太耗时间，因而
无法在main gui 线程上运行它们。相反，你可以在后台运行 tokio 运行时来生成网络请求，并在请求完成将任务信息发送回GUI线程代码，如果你想要进度条(效果)
，甚至可以增量发送。

在这个例子中，运行时配置 [multi_thread](https://docs.rs/tokio/1/tokio/runtime/struct.Builder.html#method.new_multi_thread) 是很重要的。
如果你将它改为 `current_thread` 运行时，你会发现耗时的任务会在任何后台任务开始之前完成。这是因为在 `current_thread` 上产生的后台任务，只会在调用
`block_on` 期间运行，否在运行时没有任何地方可以运行它们。

例子，通过调用 [spawn](https://docs.rs/tokio/1/tokio/runtime/struct.Runtime.html#method.spawn) 返回的 [JoinHandle](https://docs.rs/tokio/1/tokio/task/struct.JoinHandle.html)
对象上的 `block_on` 方法来等待生成任务的完成，但这也并非唯一方法。这里还有一些其它的替代方案:

* 使用消传递通道，比如: [tokio::sync::mpsc](https://docs.rs/tokio/1/tokio/sync/mpsc/index.html) 。
* 修改一个受保护的值，比如 `Mutex` 对于GUI中的进度条来说，这会是一个很好的方法，其中GUI的每一帧读取共享值。

`spawn` 方法也可用于 [Handle](https://docs.rs/tokio/1/tokio/runtime/struct.Handle.html) 类型。可以clone `handle` 类型来获得运行时的多个句柄，
每一个 `handle` 可被用于在运行时上产生新的任务。

#### 发送消息(Sending messages)
第三种技术是生成一个运行时(Runtime)并使用消息传递与其通信。它是一种最灵活的方式，你可以在下面找到一个基本的使用示例:

```rust
se tokio::runtime::Builder;
use tokio::sync::mpsc;

pub struct Task {
    name: String,
    // info that describes the task
}

async fn handle_task(task: Task) {
    println!("Got task {}", task.name);
}

#[derive(Clone)]
pub struct TaskSpawner {
    spawn: mpsc::Sender<Task>,
}

impl TaskSpawner {
    pub fn new() -> TaskSpawner {
        // Set up a channel for communicating.
        // 设置一个用于沟通的 channel
        let (send, mut recv) = mpsc::channel(16);

        // Build the runtime for the new thread.
        //
        // The runtime is created before spawning the thread
        // to more cleanly forward errors if the `unwrap()`
        // panics.
        // 为新线程构造一个 运行时(runtime)
        // 运行时在 线程之前创建出来可以更清楚的来传递错误，如果使用了 unwrap() panics 的话。
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        std::thread::spawn(move || {
            rt.block_on(async move {
                while let Some(task) = recv.recv().await {
                    tokio::spawn(handle_task(task));
                }

                // Once all senders have gone out of scope,
                // the `.recv()` call returns None and it will
                // exit from the while loop and shut down the
                // thread.
                // 一旦所有的 sender 超出作用域时，`.recv()` 的调用会返回None， 它将退出 while 循环并关闭线程
            });
        });

        TaskSpawner {
            spawn: send,
        }
    }

    pub fn spawn_task(&self, task: Task) {
        match self.spawn.blocking_send(task) {
            Ok(()) => {},
            Err(_) => panic!("The shared runtime has shut down."),
        }
    }
}
```

这个示例可以通过多种方式来配置。比如，你可以使用 [Semaphore](https://docs.rs/tokio/1/tokio/sync/struct.Semaphore.html) 信息量来限制活动的任务数量，
或者你可以使用相反方向的channel向 spawner 发送响应。当你以这种方式生成运行时时， 它是一个 [actor](https://ryhl.io/blog/actors-with-tokio/) 类型。



&larr; [主题](Topics.md)

&rarr; [优雅关机](GracefulShutdown.md)