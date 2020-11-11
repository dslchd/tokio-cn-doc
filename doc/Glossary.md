## 词汇表(Glossary)
### 异步(Asynchronous)
在Rust的上下文中，异步代码指的是使用了`async/await`语言特性的代码，该功能允许很多任务在几个线程(甚至单个线程)上同时运行.

### 并发与并行(Concurrency and parallelism)
并发与并行是两个相关的概念，在谈论到同时执行多个任务时都会使用. 如果某件事并行发生，那么它也是同时发生，但事实上并非如此：在两个任务之间
交替操作，但从来没同时执行这两个任务，这种情况是并发而不是并行.

### Future
Future 是存储某些操作当前状态的值. Future也有轮询方法，该方法使操作可以继续进行，直到需要等待的某些内容(比如网络连接)为止. 调用`poll`方法
应该很快返回.

Future通常可以在异步块中使用`.await`组合多个future来创建.

### 执行器与调度器(Executor/scheduler)
执行器与调度器通过重复调用`poll`方法来执行Future. 标准库中并没有执行器，所以你需要额外的库，并且Tokio的**运行时**提供了使用最广泛的执行器.

执行器可以在几个线程上同时运行大量的Future. 它通过在等待时交换当前正在执行的任务来执行此操作. 如果代码很长时间也没有达到`.await`，则称为
"阻塞了线程"或者"没有回到执行器"，这将阻塞其它任务的运行.

### 运行时(Runtime)
**运行时**是一种包含执行程序和与执行程序集成的各种实用程序库，比如计时器程序与IO. 运行时与执行器的名称有时候可以交换来使用. 标准库中没有运行时，
因此你要使用它就得添加额外的库，并且使用最广泛的运行时是Tokio运行时.

运行时也被用在其它上下文中，比如说，"Rust没有运行时" 的短语有时候表示Rust程序的执行没有垃圾回收或者即时编译.

### 任务(Task)
一个任务它是一个运行在Tokio运行时上的操作，它被`tokio::spawn`或者`Runtime::block_on`函数创建. 通过组合它们来创建Future的工具，比如，
`.await`和`join!`不创建新的任务，每个合并的部分都被称为"在同一个任务中".

并行性需要多个任务，但是可以使用比如`join!`之类的工具同时对一项任务执行多个操作.

### 产生(Spawning)
Spawning被表示在使用`tokio::spawn`函数来创建一个新任务. 它在标准库 [std::thread::spawn](https://doc.rust-lang.org/stable/std/thread/fn.spawn.html) 中也指创建新线程.

### 异步块(Async block)
异步块是一种用来创建Future运行某些代码的简便方法. 比如:

```rust
let world = async {
    println!("world");
}

let my_future = async {
    println!("Hello");
    world.await;
}
```

上面的代码创建了一个叫作`my_future`的Future，它会打印出`Hello world!`. 它会首先打印出Hello，然后再运行`world` future. 注意上面的代码
不会自己打印出任何的内容 - 你必须在发生一些事之前实际的执行`my_future`，方法是直接spawning它，或者在有spawning的地方使用`.await`.

### 异步函数(Async function)
与异步块类似，一个异步函数是创建函数体成为future的一种便捷方法. 所有的异步函数都可以被重写为返回future的普通函数:

```rust
async fn do_stuff(i: i32) -> String {
    // do stuff
    format!("The integer is {}.", i)
}
```

```rust
use std::future::Future;

// 上面的异步函数可以用以下同种方式表述:
fn do_stuff(i: i32) -> impl Future<Output = String> {
    async move {
        // do stuff
        format!("The integer is {}.", i)
    }
}
```

这里使用了[impl Trait syntax](https://doc.rust-lang.org/book/ch10-02-traits.html#returning-types-that-implement-traits) 语法来返回一个future，
因为[Future](https://doc.rust-lang.org/stable/std/future/trait.Future.html)是一个trait. 请注意，因为异步块创建的future在执行之前不会
执行任何操作，因此调用异步函数在返回的future被执行前也不会执行任何操作([ignoring it triggers a warning](https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=4faf44e08b4a3bb1269a7985460f1923)).

### Yielding
在Rust异步上下文中，yield指允许执行器在单个线程上执行很多的future. future的每一次出让(yield)，执行器都会使用另外一个future交换当前future，
通过这种重复的交换当前任务，执行器就可以同时的(并发的)执行大量的任务. future仅能使用`.await`操作符来yield，因此两次`.await`操作之间花费很长时间
的future就可能阻塞其它任务的执行.

具体来说，从[poll](https://doc.rust-lang.org/stable/std/future/trait.Future.html#method.poll) 方法返回时，future就会yields.

### 阻塞(Blocking)
单词 "blocking" (阻塞) 有两种不同的使用方式: "阻塞"的第一层意思是等待某些事完成，"阻塞"的另外一层含义是指当一个future花费很长时间而没有yielding时.
为了明确起见，可以将短语"blocking the thread"(阻塞线程)用于第二种含义.

在Tokio的文档中总是使用第二种"阻塞"的含义.

要在Tokio中运行阻塞的代码，请参考Tokio API指引中的[CPU-bound tasks and blocking code](https://docs.rs/tokio/0.3/tokio/#cpu-bound-tasks-and-blocking-code) 章节.

### 流(Stream)
[Stream](https://docs.rs/tokio/0.3/tokio/stream/trait.Stream.html) 是 [Iterator](https://doc.rust-lang.org/stable/std/iter/trait.Iterator.html) 的异步版本，
并提供值流. 通常与`while let` 循环一起使用，如下所示:

```rust
use tokio::stream::StreamExt; // for next()

while let Some(item) = stream.next().await {
    // do something
}
```

单词`stream`有时候用来指 [AsyncRead](https://docs.rs/tokio/0.3/tokio/io/trait.AsyncRead.html) 和 [AsyncWrite](https://docs.rs/tokio/0.3/tokio/io/trait.AsyncWrite.html) 有点令人困惑.

### 通道(Channel)
通道是一种允许一部分代码发送消息到另外一部分的工具. Tokio提供了一些通道，每一种都有其目的与用途.

* [mpsc](https://docs.rs/tokio/0.3/tokio/sync/mpsc/index.html) : 多生产者，单消费者通道. 可以发送许多的值.
* [oneshot](https://docs.rs/tokio/0.3/tokio/sync/oneshot/index.html) : 单生产者，单消费者通道. 能发送单一值.
* [broadcast](https://docs.rs/tokio/0.3/tokio/sync/broadcast/index.html) : 多生产者，多消费者通道. 发送很多值，每一个接收者都能看到每一个值.
* [watch](https://docs.rs/tokio/0.3/tokio/sync/watch/index.html) : 单生产者，多消费者通道，可以发送许多值，但不会保留历史记录. 接收者仅能看到最近的值.

如果你需要使用多生产者，多消费者通道，且仅能有一个消费者能看到每一个消息，你可以使用 [async-channel](https://docs.rs/async-channel/) 包.

还有一些在异步Rust之外使用的通道，比方说 [std::sync::mpsc](https://doc.rust-lang.org/stable/std/sync/mpsc/index.html) 和 [crossbeam::channel](https://docs.rs/crossbeam/latest/crossbeam/channel/index.html) .
这些channels 通过阻塞线程来等待消息，在这异步代码中是不被允许的.

### 背压(Backpressure)
背压是一种用来设计高负载低延迟响应，应用的一种模式. 比如说，`mpsc` 以有界和无界的形式出现. 通过使用有界通道，如果接收方无法及时处理发送方发送的消息数量时，
接收方可以对发送方施加 "背压"，这可以避免内存的使用量一直增长不受限制，因为通道上发送的消息越来越多了.

### Actor
一种设计应用程序的设计模式. Actor是指独立产生的任务，该任务代表应用程序的其它部分使用channel与应用程序的另外部分通信来管理某些资源.

有关 actor的示例，请参考 [通道](Channels.md) 章节.

&larr; [指南](../README.md)