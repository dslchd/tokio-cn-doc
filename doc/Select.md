## Select
到目前为止，当我们想向系统添加并发时，我们会产生一个新的任务(task). 现在我们将介绍使用Tokio来并发执行异步代码的其它方法.

## `tokio::select!`
`tokio::select!` 宏允许等待多个异步计算且当单个计算完成时返回(译者注: 多个并发或并行异步计算任务，返回最先完成的那个).

比如说:

```rust
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        let _ = tx1.send("one");
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
}
```

使用了两个 `oneshot` 通道. 其中任一通道都能先完成. `select!` 语句在两个channels上等待,并将`va1`绑定到任务返回的值上. 当其中任一 `tx1` 或者
`tx2` 完成时，与之相关的块就会执行.

另外没有被完成的分支将会被丢弃(dropped). 在上面的示例中，计算正在每个channel的 `oneshot::Receiver` 上等待. 没有完成的`oneshot::Receiver`
channel将会被丢弃.

### 取消(Cancellation)
对于异步Rust来说，取消操作是通过删除一个future来完成的. 回顾一下 [深入异步](AsyncInDepth.md) 章节中，使用future来实现Rust的异步操作且
future是惰性的. 仅仅当future被轮询时操作才会处理. 如果future被删除(丢弃)，操作就不会继续，因为与之所有相关联的状态都被丢弃了.

也说是说，有时候异步操作将产生后台任务或者启动在后台运行的其它操作. 比方说，在上面的示例中，产生一个任务将消息发送回去. 一般来说这个任务会执行
一些计算来生成值.

Futures或者其它类型能通过实现 `Drop` 去清理后台资源. Tokio的`oneshot::Receiver`通过向`Sender`方发送一个关闭的通知来实现`Drop`功能.
Sender方能接收到这个通知并通过丢弃正在进行的操作来中止它.

```rust
use tokio::sync::oneshot;

async fn some_operation() -> String {
    // 这里计算值
}

#[tokio::main]
async fn main() {
    let (mut tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        // select 操作和 oneshot 的 `close()` 通知.
        tokio::select! {
            val = some_operation() => {
                let _ = tx1.send(val);
            }
            _ = tx1.closed() => {
                // `some_operation()` 被调用, 
                // 任务完成且 `tx1` 被丢弃
            }
        }
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
}
```

### `Future`的实现(The `Future` implementation)
为了帮助更好的理解`select!`是如何工作的，让我们看看假想的Future实现像什么样子. 这是一个简单的版本. 在具体的实践中，`select!`还包括其它的功能，
比如像随机选择要首先轮询的分支.

```rust
use tokio::sync::oneshot;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct MySelect {
    rx1: one
}
```