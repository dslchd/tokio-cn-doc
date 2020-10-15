## 通道(Channels)
现在我们已经学习了一些与Tokio相关的并发知识, 让我们将这些知识应用到客户端. 假设我们想运行两个并发的Redis命令. 我们可以为每一个命令产生
一个任务来处理. 然后这两个命令将同时发生.

首写我们可能尝试写像下面这样的代码:

```rust
use mini_redis::client;

#[tokio::main]
async fn main() {
    // 建立一个与Server的链接
    let mut client = client::connect("127.0.0.1:6379").await.unwrap();

    // 产生两个任务, 一个获取key, 另外一个设置key值.
    let t1 = tokio::spawn(async {
        let res = client.get("hello").await;
    });

    let t2 = tokio::spawn(async {
        client.set("foo", "bar".into()).await;
    });

    t1.await.unwrap();
    t2.await.unwrap();
}
```

上面的代码不能被编译, 因为两个任务都需要以某种方式访问 `client` . 而 `client` 没有实现 `Copy` , 因此如果没有一些可以促进"共享"的代码, 它将
无法编译. 另外, `Client::set` 需要 `&mut self`, 这意味着需要独占的访问权才能调用它. 我们可以为每一个任务打开一个链接,但这不是一个好办法.
我们不能使用 `std::sync::Mutex` , 因为　`.await` 需要在持有锁的情况下调用. 我们可以使用　`tokio::sync::Mutex` , 但是这样又仅允许一个
进行中的请求. 如果客户端实现 [pipelining](https://redis.io/topics/pipelining) , 异步互斥锁又不能充分的利用链接了.

## 消息传递(Message passing)
结论就是使用消息传递机制. 该模式涉及产生一个专门的任务来管理 `client` 中的资源. 任何希望发出请求的任务都会向`client` 的任务发送一条消息.
`client` 任务代表发送方发出请求, 并将响应返回给发送方.

使用这种策略,可以建立单个的链接. 管理 `client` 的任务可以获取独占访问权, 以便来调用　`get` 和　`set` . 另外, 通道还用作缓冲区. 
客户端任务比较繁忙的时候,可能会将操作发送到客户端任务. 一旦 `client` 任务可以用来处理新链接, 它将从通道中拉取下一个请求(进行处理).
这样的方式可以提高吞吐量,并可以扩展的方式来支持链接池.

## Tokio的通道原语(Tokio's channel primitives)
Tokio提供了许多通道( [number of channels](https://docs.rs/tokio/0.2/tokio/sync/index.html) ), 每一种都有其对应的用途.
* [mpsc](https://docs.rs/tokio/0.2/tokio/sync/mpsc/index.html) : 多生产者(multi-producer)单消费者(single-consumer)通道. 可以发送许多的值.
* [oneshot](https://docs.rs/tokio/0.2/tokio/sync/oneshot/index.html) : 单生产者(single-producer)单消费者(single-consumer)通道. 可以发送单个值.
* [broadcast](https://docs.rs/tokio/0.2/tokio/sync/broadcast/index.html) : 多生产者多消费者(广播). 可以发送许多值,每一个接收者都能看到每一个值.
* [watch](https://docs.rs/tokio/0.2/tokio/sync/watch/index.html) : 单生产者多消费者. 可以发送许多值,但是不会保留历史记录. 接收者仅能看到最新的值.

如果你需要一个多生产者多消费者通道且仅仅只想让一个消费者看到所有消息, 你可以使用　[async-channel](https://docs.rs/async-channel/) 包. 
在异步Rust之外还有其它通道可以使用,比如, [std::sync::mpsc](https://doc.rust-lang.org/stable/std/sync/mpsc/index.html) 和 [crossbeam::channel](https://docs.rs/crossbeam/latest/crossbeam/channel/index.html) . 这些通道通过阻塞线程来等待消息, 这在
异步代码中是不允许的.

在本章节中我们将使用　[mpsc](https://docs.rs/tokio/0.2/tokio/sync/mpsc/index.html) 与　[oneshot](https://docs.rs/tokio/0.2/tokio/sync/oneshot/index.html) .
后面的章节将讨论其它的消息通道类型. 本章完整的代码可以在 [这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/channels/src/main.rs)　找到.

## 定义消息类型(Define the message type)
在大多数情况下, 使用消息传递时, 接收消息的任务会响应多个命令. 在我们的案例中, 任务将响应　`GET` 与　`SET` 命令. 为了对这个建模,我们首先
定义一个 `Command` 的枚举, 并为每种命令类型包含一个变体.

```rust
use bytes::Bytes;

#[derive(Debug)]
enum Command {
    Get {
        key: String,
    },
    Set {
        key: String,
        val: Bytes,
    }
}
```

## 创建通道(Create the channel)
在　`main`　中　创建　`mpsc`　通道.

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // 创建一个最大容量为32的通道
    let (mut tx, mut rx) = mpsc::channel(32);

    // ... Rest comes here
}
```

`mpsc` 通道被用来发送一个命令到管理Redis链接的任务中. 多生产者的能力是能让许多的任务发送消息. 创建的通道返回两个值, 一个是发送者(Sender)一个是
接收者(receiver). 它们两者被分开使用. 他们可能移动到不同的任务中去.

被创建的通道容量为32. 如果消息的发送速度大于接收的速度, 通道会储存它们. 一旦通道中存了32条消息时,就会调用　`send(...).await` 进入睡眠状态,
直到接收者删除一条消息为止.(译者注: 就是说当接收者有能力能再次处理消息时, 睡眠状态才会结束).

通过 **克隆** (**cloning**) `Sender` 可以完成多个任务的发送. 比如像下面这样:

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = mpsc::channel(32);
    let mut tx2 = tx.clone();

    tokio::spawn(async move {
        tx.send("sending from first handle").await;
    });

    tokio::spawn(async move {
        tx2.send("sending from second handle").await;
    });

    while let Some(message) = rx.recv().await {
        println!("GOT = {}", message);
    }
}
```

两条消息都发送到单个　`Receiver` 处理. 不可能克隆　`mpsc` 通道中的接收者.

当每个 `Sender` 超出作用域范围或者被dropped时, 它不能再发送更多的消息到通道中. 此时, `Receiver` 上的　`rev`　调用都将返回 `None` ,
这意味着所有发送者都已经消失且通道已关闭.

在我们管理Redis链接的任务中,它知道一旦通道关闭就能关闭Redis链接, 因为该链接不再使用了.

## 产生管理任务(Spawn manager task)
下一步, 产生一个任务来处理来自通道的消息. 首先, 建立与Redis的链接. 然后, 通过Redis链接发出接收到的命令.

```rust
use mini_redis::client;
// move 关键字用来移动　rx 所有权到task中去
let manager = tokio::spawn(async move {
    // 建立与Server的链接
    let mut client = client::connect("127.0.0.1:6379").await.unwrap();

    // 开始接收消息
    while let Some(cmd) = rx.recv().await {
        use Command::*;

        match cmd {
            Get { key } => {
                client.get(&key).await;
            }
            Set { key, val } => {
                client.set(&key, val).await;
            }
        }
    }
});
```

现在,更新这两个任务来使用通道发送命令,而不是直接在Redis的链接上发出命令.

```rust
// Sender 被移动到task中了, 这里有两个任务, 所以我们需要第二个　Sender
let mut tx2 = tx.clone();

// 产生两个任务一个得到key值,一个设置key的值
let t1 = tokio::spawn(async move {
    let cmd = Command::Get {
        key: "hello".to_string(),
    };

    tx.send(cmd).await.unwrap();
});

let t2 = tokio::spawn(async move {
    let cmd = Command::Set {
        key: "foo".to_string(),
        val: "bar".into(),
    };

    tx2.send(cmd).await.unwrap();
});
```

## 接收响应
最后一步就是接收来管理任务的响应. `GET` 命令需要获取值, 而 `SET` 命令需要知道操作是否完成. 

为了传递响应,可以使用 `oneshot` 通道. `oneshot` 通道是一个经过了优化的单生产者单消费者通道,用来发送单个值. 在我们的案例中,单个值就是响应.

与 `mpsc` 类似, `oneshot` 返回一个发送者(Sender)和一个接收者(receiver)处理器.

```rust
use tokio::sync::oneshot;

let (tx,rx) = oneshot::channel();
```

与 `mpsc` 不同, `oneshot` 它不能指定任何容量, 因为容量始终为1. 另外, 两个处理器都不能被克隆(译者注: 指 tx, rx).

为了接收到来自管理任务的响应, 在发送一个命令之前, 一个 `oneshot` 通道将被创建. 通道 `Sender` 的一半包含在管理任务的命令中. 接收方的一半用来接收响应.

首先, 更新 `Command` 来包含一个　`Sender` . 为了方便, 为 `Sender` 定义一个类型别名.

```rust
use tokio::sync::oneshot;
use bytes::Bytes;

/// 多个不同的命令在单个通道上复用.
#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Vec<u8>,
        resp: Responder<()>,
    },
}

/// 由请求者提供并通过管理任务来发送,再将命令的响应返回给请求者.
type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;
```

现在,更新发出命令的任务来包括 `oneshot::Sender` .

```rust
let t1 = tokio::spawn(async move {
    let (resp_tx, resp_rx) = oneshot::channel();
    let cmd = Command::Get {
        key: "hello".to_string(),
        resp: resp_tx,
    };

    // 发送 GET 请求
    tx.send(cmd).await.unwrap();

    // 等待响应结果
    let res = resp_rx.await;
    println!("GOT = {:?}", res);
});

let t2 = tokio::spawn(async move {
    let (resp_tx, resp_rx) = oneshot::channel();
    let cmd = Command::Set {
        key: "foo".to_string(),
        val: b"bar".to_vec(),
        resp: resp_tx,
    };

    // 发送 GET 请求
    tx2.send(cmd).await.unwrap();

    // 等待响应结果
    let res = resp_rx.await;
    println!("GOT = {:?}", res)
});
```

最后, 更新管理任务以通过oneshot通道发送响应.

```rust
while let Some(cmd) = rx.recv().await {
    match cmd {
        Command::Get { key, resp } => {
            let res = client.get(&key).await;
            // 忽略错误
            let _ = resp.send(res);
        }
        Command::Set { key, val, resp } => {
            let res = client.set(&key, val.into()).await;
            // 忽略错误
            let _ = resp.send(res);
        }
    }
}
```

在 `oneshot::Sender` 上调用 `send` 会立即完成而不需要 `.await` 操作. 这是因为在 `oneshot` 通道上的 `send` 总是立即失败或者成功,
而没有任何等待.

当接收一半时删除(dropped)了, 在 `oneshot` 通道上发送一个值会返回 `Err` . 这表明接收方不再对响应有兴趣,在我们的方案中, 接收方的取消操作
是可以被接受的事件. `resp.send(...)` 返回的 `Err` 不需要处理.

你可以在 [这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/channels/src/main.rs) 找到完整的代码.

## 背压与通道边界(Backpressure and bounded channels)
每当引用并发或队列时, 最重要的是确保队列是有界的, 且系统会优雅的处理负载. 无界队列最终将占用所有的内存,并导致系统以无法预测的方式发生故障.

Tokio 比较注意避免隐式(无界)队列. 其中很大一部分原因是异步操作是惰性的. 考虑如下代码:

```rust
loop {
    async_op();
}
```

如果异步操作非常急切的运行, 在没有确保先前操作已经完成的情况下, loop 循环将会重复入队一个新的 `async_op` 来运行. 这就导致隐式无界队列的产生.
基于回调的系统与基于feature系统尤其容易受到这样的影响.

然而,使用Tokio和异步Rust, 上面的代码片段根本不会运行 `async_op` . 这是因为你没有调用 `.await` . 如果代码片段更新一下变为使用 `.await` , 
则 loop 循环将在重新开始之前等待上一个操作完成.

```rust
loop {
    // 不会重复 直到 async_op 操作完成
    async_op().await;
}
```

要明确的引用并发与队列,做到这一点的方法包括:

* `tokio::spawn`
* `select!`
* `join!`
* `mpsc::channel`

当这样做时,请确保一定数量的并发总量. 比如说, 在编写TCP接收循环时, 要确保打开的socket链接总数是有界的. 当使用 `mspc::channel` 时, 要选择
一个可管理的通道容量(译者注: 就是要设置一个确定的容量数).  特定的界限值将取决于应用程序.

注意并选择(或设置)良好的边界是编写可靠Tokio应用的重要组成部分.



&larr; [共享状态](sharedState.md)

&rarr; [I/O](IO.md)

