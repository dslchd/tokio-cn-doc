## 通道(Channels)
现在我们已经学习了一些与Tokio相关的并发的知识, 让我们将这些知识应用到客户端. 假设我们想运行两个并发的Redis命令. 我们可以为每一个命令产生
一个任务来处理. 然后这两个命令将同时发生.

首写我们可能尝试像下面这样写:

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
无法编译. 另外, `Client::set` 需要 `&mut self`, 这意味着需要独占的访问权才能调用它. 我们可以为每一个任务打一个链接,但这不是一个好办法.
我们不能使用 `std::sync::Mutex` , 因为　`.await` 需要在持有锁的情况下调用. 我们可以使用　`tokio::sync::Mutex` , 但是这样又仅允许一个
进行中的请求. 如果客户端实现 [pipelining](https://redis.io/topics/pipelining) , 异步互斥锁又不能充分的利用链接了.

## 消息传递(Message passing)
结论就是使用消息传递机制. 该模式涉及产生一个专门的任务来管理 `client` 中的资源. 任何希望发出请求的任务都会向　`client` 的任务发送一条消息.
`client` 任务代表发送方发出的请求, 并将响应返回给发送方.

使用这种策略,可以建立单个的链接. 管理 `client` 的任务可以获取独占访问权, 以便来调用　`get` 和　`set` . 另外, 该通道还用作缓冲区. 
客户端任务比较繁忙的时候,可能会将操作发送到客户端任务. 一旦 `client` 任务可以用来处理新链接, 它将从通道中拉取下一个请求(进行处理).
这样的方式可以提高吞吐量,并可以扩展的方式来支持链接池.

## Tokio的通道原语(Tokio's channel primitives)
Tokio提供了许多通道( [number of channels](https://docs.rs/tokio/0.2/tokio/sync/index.html) ), 每一种都有其对应的用途.
* [mpsc](https://docs.rs/tokio/0.2/tokio/sync/mpsc/index.html) : 多生产者(multi-producer)单消费者(single-consumer)通道. 可以发送许多的值.
* [oneshot](https://docs.rs/tokio/0.2/tokio/sync/oneshot/index.html) : 单生产者(single-producer)单消费者(single-consumer)通道. 可以发送单个值.
* [broadcast](https://docs.rs/tokio/0.2/tokio/sync/broadcast/index.html) : 多生产者多消费者(广播). 可以发送许多值,每一个接收者都能看到每一个值.
* [watch](https://docs.rs/tokio/0.2/tokio/sync/watch/index.html) : 单生产者多消费者. 可以发送许多值,但是不会保留历史记录. 接收者仅能看到最新的值.

如果你需要一个多生产者多消费者通道且仅仅只想一个消费者看到所有消息, 你可以使用　[async-channel](https://docs.rs/async-channel/) 包. 
在异步Rust之外还有其它通道可以使用,比如, [std::sync::mpsc](https://doc.rust-lang.org/stable/std/sync/mpsc/index.html) 
和　[crossbeam::channel](https://docs.rs/crossbeam/latest/crossbeam/channel/index.html) . 这些通道通过阻塞线程来等待消息, 这在
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

`mpsc` 通道被用来发送一个命令到管理Redis链接的任务中. 多生产者的能力是允许任务发送消息. 创建的通道返回两个值, 一个是发送者(Sender)一个是
接收者(receiver). 它们两者被分开使用. 他们可能移动到不同的任务中去.

被创建的通道容量为32. 如果消息的发送速度大于接收的速度, 通道会储存它们. 一旦通道中存了32条消息时,就会调用　`send(...).await` 进入睡眠状态,
直到接收者删除一条消息为止.(译者注: 就是说只有接收者有力能再次处理消息时,睡眠状态才会结束).

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

当每个 `Sender` 超出作用域范围或者被dropped时, 它不再能发送更多的消息到通道中. 此时, `Receiver` 上的　`rev`　调用都将返回 `None` ,
这意味着所有发送者都已经消息且通道已关闭.

在我们管理Redis链接的任务中,它知道一旦通道关闭就能关闭Redis链接, 因为该链接不再使用了.

## 产生任务管理(Spawn manager task)
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

