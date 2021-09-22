## 优雅关机(Graceful Shutdown)
这篇文章的目的是告诉你如何在异步应用中正常的关机。

要实现优雅关机一般有3个部分:

* 确定何时关机
* 告诉程序每一部分关闭
* 等待程序的其它部分关闭

文章的剩余部分将介绍这三部分。可以在 [mini-redis](https://github.com/tokio-rs/mini-redis/) 中找到真实世界
如何正确关机的实现，特别是在 [src/server.rs](https://github.com/tokio-rs/mini-redis/blob/master/src/server.rs) 和 
[src/shutdown.rs](https://github.com/tokio-rs/mini-redis/blob/master/src/shutdown.rs) 文件中有.

### 确定何时关机(Figuring out when to shut down)
这一点肯定是取决于应用程序，但有一个很关键的标准是应用程序从操作系统接收一个信号。这种情况发生在，当你的应用程序运行在终端时按 `ctrl+c`
时。为了侦探到这个信息号，`Tokio` 提供了一个 [tokio::signal::ctrl_c](https://docs.rs/tokio/1/tokio/signal/fn.ctrl_c.html) 函数，
你可以像下面这样来使用它:

```rust
use tokio::signal;

#[tokio::main]
async fn main() {
    // ... 产生一个其它任务 task ...

    match signal::ctrl_c().await {
        Ok(()) => {},
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
            // 关闭的时候也可能出现错误
        },
    }

    // 发送一个关机信号给应用程序并等待
}
```

如果你有多个关机条件，你可以使用 [mpsc channel](https://docs.rs/tokio/1/tokio/sync/mpsc/index.html) 来将关机信息发送到一个地方。
然后你可以在channel上通过 [Select](https://docs.rs/tokio/1/tokio/macro.select.html) 匹配到 `ctrl_c` 信号。比如像下面这样:

```rust
use tokio::signal;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (shutdown_send, shutdown_recv) = mpsc::unbounded_channel();

    // ... 产生一个其它任务 task ...
    //
    // application uses shutdown_send in case a shutdown was issued from inside
    // 应用使用 shutdown_send 发出关机信息来防止应用从内部关闭
    // the application

    tokio::select! {
        _ = signal::ctrl_c() => {},
        _ = shutdown_recv.recv() => {},
    }

    // 发送一个关机信号给应用程序并等待
}
```

### 告之关机的一些事情(Telling things to shut down)
告诉应用程序每一部分关闭时常用的工具是 [broadcast channel](https://docs.rs/tokio/1/tokio/sync/broadcast/index.html) 。
想法其实很简单，应用程序中的每一个任务都有一个广播(broadcast) 通道(channel)接收器，当消息在channel上广播时，任务会自行关闭。通常，
使用 [tokio::select](https://docs.rs/tokio/1/tokio/macro.select.html) 来接收这个广播消息。比如在 `mini-redis` 的每一个
任务中来接收 `shutdown` 消息的方式:

```rust
let next_frame = tokio::select! {
    res = self.connection.read_frame() => res?,
    _ = self.shutdown.recv() => {
        // If a shutdown signal is received, return from `run`.
        // 如果一个 shutdown 信号被接收到，将从 `运行` 状态返回，并将导致此任务终止.
        // This will result in the task terminating.
        return Ok(());
    }
};
```

在 `mini-redis` 的示例中，当一个关机信号被接收到时，task(任务)会立即终止，但有时候你需要在终止任务之前运行一个`关机过程`。比方说，
有时候你需要在关机前将数据刷到一个文件或数据库中，或者有任务管理的链接，你可能想在任务终止前在链接上发送关机消息。

有一个很好的方式是，将 `broadcast channel` 包装到一个 struct 中。这里有一个示例 [这里](https://github.com/tokio-rs/mini-redis/blob/master/src/shutdown.rs) 。

值得一提的是你也可以使用 [watch channel](https://docs.rs/tokio/1/tokio/sync/watch/index.html) 来达到同样的效果。这两种方式之间没有明显的差异。

### 等待一些事情完成关闭(Waiting for things to finish shutting down)

一旦你告诉另一个任务要关闭时，你需要等待它们完成。最简单的方法是使用 [mpsc channel](https://docs.rs/tokio/1/tokio/sync/mpsc/index.html)
这里不是发送消息，而是等待通道的关闭，这时每一个sender都会被丢弃。

下面是上面这种方式的简单示例，示例生成10个任务，然后使用 `mpsc` 通道等待它们关闭。

```rust
use tokio::sync::mpsc::{channel, Sender};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let (send, mut recv) = channel(1);

    for i in 0..10 {
        tokio::spawn(some_operation(i, send.clone()));
    }

    // 等待此任务task完成
    //
    // We drop our sender first because the recv() call otherwise
    // 我们丢弃drop掉 sender 是因为 recv()的调用，不然的话将会一直休眠
    // sleeps forever.
    drop(send);

    // When every sender has gone out of scope, the recv call
    // 当每人个 sender 超过作用域时，recv 的调用将返回error。这里我们忽略它。
    // will return with an error. We ignore the error.
    let _ = recv.recv().await;
}

async fn some_operation(i: u64, _sender: Sender<()>) {
    sleep(Duration::from_millis(100 * i)).await;
    println!("Task {} shutting down.", i);

    // sender 离开了作用域 ...
}
```

有个很重要的点是，等待关闭的任务都持有一个sender. 在这种情况下你必须确保等待通道关闭之前删除此sender。

&larr; [同步代码桥接](BridgingWithSyncCode.md)

&rarr; [词汇表](Glossary.md)