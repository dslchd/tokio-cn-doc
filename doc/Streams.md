## 流(Streams)
流是一个一系列异步值的称呼. 它与Rust的 [std::iter::Iterator](https://doc.rust-lang.org/book/ch13-02-iterators.html) 异步等效且由
[Stream](https://docs.rs/tokio/0.3/tokio/stream/trait.Stream.html) trait表示. 流能在`async`函数中迭代. 它们也可以使用适配器进行
转换. Tokio在 [StreamExt](https://docs.rs/tokio/0.3/tokio/stream/trait.StreamExt.html) trait上提供了一些通用适配器.

Tokio 在 `stream` 特性标识下提供对流的支持. 当依赖Tokio时, 包括`stream`或`full`的特性能访问此功能.

```toml
tokio ={version = "0.3", features = ["stream"]}
```

我们已经看到了一些类型也实现了[Stream](https://docs.rs/tokio/0.3/tokio/stream/trait.Stream.html). 比如说，[mpsc::Receiver](https://docs.rs/tokio/0.3/tokio/sync/mpsc/struct.Receiver.html)
的接收(receive)部分也实现了`Stream`. [AsyncBufReadExt::lines()](https://docs.rs/tokio/0.3/tokio/io/trait.AsyncBufReadExt.html#method.lines)
方法采用一个被缓存的 I/O reader并返回一个 `Stream`，其中每个值代表一行数据.

## 迭代(Iteration)
当前Rust程序语言还不支持异步`for`循环. 取而代之是的使用`while let`循环与 [StreamExt::next()](https://docs.rs/tokio/0.3/tokio/stream/trait.StreamExt.html#method.next) 配对来完成流的迭代.

```rust
use tokio::stream::StreamExt;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = mpsc::channel(10);

    tokio::spawn(async move {
        tx.send(1).await.unwrap();
        tx.send(2).await.unwrap();
        tx.send(3).await.unwrap();
    });

    while let Some(v) = rx.next().await {
        println!("GOT = {:?}", v);
    }
}
```

像迭代器一样，`next()`方法返回`Option<T>` 其中 `T` 就是流的类型. 接收到 `None` 表明流迭代被终止了.

### Mini-Redis 广播(Mini-Redis broadcast)
让我们来看一个使用Mini-Redis客户端的更加复杂的示例.

完整的代码可以看 [这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/streams/src/main.rs).

```rust
use tokio::stream::StreamExt;
use mini_redis::client;

async fn publish() -> mini_redis::Result<()> {
    let mut client = client::connect("127.0.0.1:6379").await?;

    // Publish some data
    client.publish("numbers", "1".into()).await?;
    client.publish("numbers", "two".into()).await?;
    client.publish("numbers", "3".into()).await?;
    client.publish("numbers", "four".into()).await?;
    client.publish("numbers", "five".into()).await?;
    client.publish("numbers", "6".into()).await?;
    Ok(())
}

async fn subscribe() -> mini_redis::Result<()> {
    let client = client::connect("127.0.0.1:6379").await?;
    let subscriber = client.subscribe(vec!["numbers".to_string()]).await?;
    let messages = subscriber.into_stream();

    tokio::pin!(messages);

    while let Some(msg) = messages.next().await {
        println!("got = {:?}", msg);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> mini_redis::Result<()> {
    tokio::spawn(async {
        publish().await
    });

    subscribe().await?;

    println!("DONE");

    Ok(())
}
```

产生一个任务来将消息发布到"numbers" 通道上的Mini-Redis服务上. 然后我们在main(主)任务中，订阅"numbers" 通道并显示收到的消息.

订阅之后，在返回的订阅者上调用 [into_stream()](https://docs.rs/mini-redis/0.3/mini_redis/client/struct.Subscriber.html#method.into_stream).
消息者订阅，返回在消息到达时产生消息的流. 在我们开始迭代消息之前，注意到，使用了`tokio::pin!`将流固定到堆栈. 在流上调用`next()`需要流被
[pinned](https://doc.rust-lang.org/std/pin/index.html). `into_stream()` 函数返回的流不是固定的，我们必须显示的固定住它来进行迭代.

```markdown
一个Rust的值是"pinned"时，它会被固定且它不能在内存中移动. 固定值的关键是可以将指针用作固定数据，并且调用都可以确信指针一直保持有效.
`async/await`使用此特性来支持跨`.await`点的数据**借用**.
```

如果我们忘记固定住流，我们会得到像下面这样的错误:

```text
error[E0277]: `std::future::from_generator::GenFuture<[static generator@mini_redis::client::Subscriber::into_stream::{{closure}}#0 0:mini_redis::client::Subscriber, 1:async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static)>>> for<'r, 's, 't0, 't1, 't2, 't3, 't4, 't5, 't6> {std::future::ResumeTy, &'r mut mini_redis::client::Subscriber, mini_redis::client::Subscriber, impl std::future::Future, (), std::result::Result<std::option::Option<mini_redis::client::Message>, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't0)>>, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't1)>, &'t2 mut async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't3)>>>, async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't4)>>>, std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't5)>>, impl std::future::Future, std::option::Option<mini_redis::client::Message>, mini_redis::client::Message}]>` cannot be unpinned
   --> streams/src/main.rs:22:36
    |
22  |     while let Some(msg) = messages.next().await {
    |                                    ^^^^ within `impl futures_core::stream::Stream`, the trait `std::marker::Unpin` is not implemented for `std::future::from_generator::GenFuture<[static generator@mini_redis::client::Subscriber::into_stream::{{closure}}#0 0:mini_redis::client::Subscriber, 1:async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static)>>> for<'r, 's, 't0, 't1, 't2, 't3, 't4, 't5, 't6> {std::future::ResumeTy, &'r mut mini_redis::client::Subscriber, mini_redis::client::Subscriber, impl std::future::Future, (), std::result::Result<std::option::Option<mini_redis::client::Message>, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't0)>>, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't1)>, &'t2 mut async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't3)>>>, async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't4)>>>, std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't5)>>, impl std::future::Future, std::option::Option<mini_redis::client::Message>, mini_redis::client::Message}]>`
    | 
   ::: /home/carllerche/.cargo/registry/src/github.com-1ecc6299db9ec823/mini-redis-0.2.0/src/client.rs:398:37
    |
398 |     pub fn into_stream(mut self) -> impl Stream<Item = crate::Result<Message>> {
    |                                     ------------------------------------------ within this `impl futures_core::stream::Stream`
    |
    = note: required because it appears within the type `impl std::future::Future`
    = note: required because it appears within the type `async_stream::async_stream::AsyncStream<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static)>>, impl std::future::Future>`
    = note: required because it appears within the type `impl futures_core::stream::Stream`

error[E0277]: `std::future::from_generator::GenFuture<[static generator@mini_redis::client::Subscriber::into_stream::{{closure}}#0 0:mini_redis::client::Subscriber, 1:async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static)>>> for<'r, 's, 't0, 't1, 't2, 't3, 't4, 't5, 't6> {std::future::ResumeTy, &'r mut mini_redis::client::Subscriber, mini_redis::client::Subscriber, impl std::future::Future, (), std::result::Result<std::option::Option<mini_redis::client::Message>, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't0)>>, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't1)>, &'t2 mut async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't3)>>>, async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't4)>>>, std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't5)>>, impl std::future::Future, std::option::Option<mini_redis::client::Message>, mini_redis::client::Message}]>` cannot be unpinned
   --> streams/src/main.rs:22:27
    |
22  |     while let Some(msg) = messages.next().await {
    |                           ^^^^^^^^^^^^^^^^^^^^^ within `impl futures_core::stream::Stream`, the trait `std::marker::Unpin` is not implemented for `std::future::from_generator::GenFuture<[static generator@mini_redis::client::Subscriber::into_stream::{{closure}}#0 0:mini_redis::client::Subscriber, 1:async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static)>>> for<'r, 's, 't0, 't1, 't2, 't3, 't4, 't5, 't6> {std::future::ResumeTy, &'r mut mini_redis::client::Subscriber, mini_redis::client::Subscriber, impl std::future::Future, (), std::result::Result<std::option::Option<mini_redis::client::Message>, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't0)>>, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't1)>, &'t2 mut async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't3)>>>, async_stream::yielder::Sender<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't4)>>>, std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 't5)>>, impl std::future::Future, std::option::Option<mini_redis::client::Message>, mini_redis::client::Message}]>`
    | 
   ::: /home/carllerche/.cargo/registry/src/github.com-1ecc6299db9ec823/mini-redis-0.2.0/src/client.rs:398:37
    |
398 |     pub fn into_stream(mut self) -> impl Stream<Item = crate::Result<Message>> {
    |                                     ------------------------------------------ within this `impl futures_core::stream::Stream`
    |
    = note: required because it appears within the type `impl std::future::Future`
    = note: required because it appears within the type `async_stream::async_stream::AsyncStream<std::result::Result<mini_redis::client::Message, std::boxed::Box<(dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static)>>, impl std::future::Future>`
    = note: required because it appears within the type `impl futures_core::stream::Stream`
    = note: required because of the requirements on the impl of `std::future::Future` for `tokio::stream::next::Next<'_, impl futures_core::stream::Stream>`

error: aborting due to 2 previous errors

For more information about this error, try `rustc --explain E0277`.
error: could not compile `streams`.

To learn more, run the command again with --verbose.
```

如果你遇到一个像这样的错误，可以尝试将流固定!

在运行之前，先启动Mini-Redis 服务:

```shell script
$ mini-redis-server
```

然后尝试运行代码，我们将看到消息在标准输出流中打印.

```text
got = Ok(Message { channel: "numbers", content: b"1" })
got = Ok(Message { channel: "numbers", content: b"two" })
got = Ok(Message { channel: "numbers", content: b"3" })
got = Ok(Message { channel: "numbers", content: b"four" })
got = Ok(Message { channel: "numbers", content: b"five" })
got = Ok(Message { channel: "numbers", content: b"6" })
```

由于订阅与发布之间存在竞争，某些早期的消息可能会被丢弃. 该程序永远不会退出. 只要服务器处于活动状态，对Mini-Redis通道的订阅将保持活动状态.

让我们来看看如何使用流来扩展此程序.

## 适配器(Adapters)
接收一个`Stream`并返回一个`Stream`的函数通常被叫做"流适配器"(Stream adapters)，因为他们是适配器模式中的一种形式. 公共流适配器包括
[map](https://docs.rs/tokio/0.3/tokio/stream/trait.StreamExt.html#method.map)，[take](https://docs.rs/tokio/0.3/tokio/stream/trait.StreamExt.html#method.take)，
和[filter](https://docs.rs/tokio/0.3/tokio/stream/trait.StreamExt.html#method.filter).

让我们更新一个Mini-Redis来让它可以退出. 在接收到三个消息之后停止迭代消息. 这可以用`take`. 此适配器限制流最多产生 `n` 个消息.

```rust
let message = subscriber.into_stream().take(3);
```

再次运行程序，我们可以看到:

```text
got = Ok(Message { channel: "numbers", content: b"1" })
got = Ok(Message { channel: "numbers", content: b"two" })
got = Ok(Message { channel: "numbers", content: b"3" })
```

这一次程序可以终止.

现在让我们将流限制为一个数字. 我们将通过消息的长度来检查. 我们使用`filter`适配器来删除与条件(译者注: predicate,这里译为条件好点)不匹配的消息.

```rust
let messages = subscriber
    .into_stream()
    .filter(|msg| match msg {
        Ok(msg) if msg.content.len() == 1 => true,
        _ => false,
    })
    .take(3);
```

再一次执行程序，我们看到:

```text
got = Ok(Message { channel: "numbers", content: b"1" })
got = Ok(Message { channel: "numbers", content: b"3" })
got = Ok(Message { channel: "numbers", content: b"6" })
```

请注意适配器的应用很重要. 首先调用`filter`然后再是`take`与调用`take`再`filter`是不同的.

最后我们通过剥离 `Ok(Message { ... }` 的输出部分来整理输出. 这是使用`map`来完成. 因为它应用在`filter`之后，我们知道消息是 `Ok`，所以
我们可以使用`unwrap()`.

```rust
let messages = subscriber
    .into_stream()
    .filter(|msg| match msg {
        Ok(msg) if msg.content.len() == 1 => true,
        _ => false,
    })
    .map(|msg| msg.unwrap().content)
    .take(3);
```

现在我们得到输出:

```text
got = b"1"
got = b"3"
got = b"6"
```

另外可选的是，组合`filter`与`map`的操作步可以使用 [filter_map](https://docs.rs/tokio/0.3/tokio/stream/trait.StreamExt.html#method.filter_map).

这里有更多可用的适配器，清单请查看[这里](https://docs.rs/tokio/0.3/tokio/stream/trait.StreamExt.html).

## `Stream`的实现(Implementing `Stream`)
[Stream](https://docs.rs/tokio/0.3/tokio/stream/trait.Stream.html) trait与 [Future](https://doc.rust-lang.org/std/future/trait.Future.html) trait非常类似.

```rust
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait Stream {
    type Item;

    fn poll_next(
        self: Pin<&mut Self>, 
        cx: &mut Context<'_>
    ) -> Poll<Option<Self::Item>>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
```

`Stream::poll_next()` 函数与`Future::poll`非常类似，不同之处在于，它可以被重复调来从流中接收多个值. 与我们在[深入异步](AsyncInDepth.md)
中看到的一样，当流不是就绪状态时将返回`Poll::Pending`. 任务注册waker程序. 一旦应该再次轮询流时，就会通知waker.

`size_hint()` 方法使用的方式与[iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html)相同.

通常当手动实现一个`Stream`时，它是通过组合future和其它流来完成的. 例如，让我们以在[深入异步](AsyncInDepth.md)中实现的`Delay` future为基础.
我们将它转换成10ms为间隔产生三次`()`的流.

```rust
use tokio::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

struct Interval {
    rem: usize,
    delay: Delay,
}

impl Stream for Interval {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<()>>
    {
        if self.rem == 0 {
            // No more delays
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.delay).poll(cx) {
            Poll::Ready(_) => {
                let when = self.delay.when + Duration::from_millis(10);
                self.delay = Delay { when };
                self.rem -= 1;
                Poll::Ready(Some(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
```

### `async-stream`
使用`Stream` trait手动来实现流可能很繁琐. 不幸的是，Rust语言目前还不支持在流上使用`async/await`语法. 这还在进行中，但现在还没准备好.(译者注: 指在流上的`async/await`语法)

[async-stream](https://docs.rs/async-stream) 包是一个临时可用的解决方案. 这个包提供了一个`async_stream!`的宏，可以将输入转换成一个流》
使用这个包，可以像这样实现上面的间隔需求:

```rust
use async_stream::stream;
use std::time::{Duration, Instant};

stream! {
    let mut when = Instant::now();
    for _ in 0..3 {
        let delay = Delay { when };
        delay.await;
        yield ();
        when += Duration::from_millis(10);
    }
}
```

&larr; [Select](Select.md)

&rarr; [词汇表](Glossary.md)


