## 共享状态(Shared state)
到目前为止, 我们有了一个能工作的键值对服务. 然后, 这里有一个主要的缺陷: 状态不能在链接之间共享. 因此我们将在这篇文章中修复这个问题.

## 策略(Strategies)
在Tokio中共享状态有两种不同的方法.
1. 使用Mutex(互斥锁)保护共享状态.
2. 产生一个任务来管理状态并使用消息传递对其进行操作.

通常来说你想使用第一种方试来处理简单的数据, 对于需要异步工作的事务(比如 I/O 原语)应该使用第二种方试. 在本章节中, 共享状态是一个 `HashMap`
并使用 `insert` 与 `get` 来操作. 这些操作都不是异步的,因此我们使用 `Mutex` .

下一章节将介绍另外一种方法.

## 添加 `bytes` 依赖(Add `bytes` dependency)
Mini-Redis包使用 `bytes` 包中的 `Bytes` 类型, 而不是使用 `Vec<u8>` . `Bytes` 的目的是为网络编程提供一个健全的字节数组结构. 它在
`Vec<u8>` 上添加的最大功能是浅克隆. 换句话说, 在 `Bytes` 的实例上调用 `clone()` 方法不会复制底层的数据. 相反 `Bytes` 实例是对一些
底层数据的引用计数. `Bytes` 类型大致与一个 `Arc<Vec<u8>>` 类似, 但还添加了一些其它功能.

为了依赖 `bytes` , 在你的 `Cargo.toml` 文件中的 `[dependencies]` 下添加如下内容:
```toml
bytes = "0.5"
```

## 初始化 `HashMap` (Initialize the `HashMap` )
`HashMap` 会在许多任务和可能的许多线程之间共享. 为了支持这一点, 它被包装在 `Arc<Mutex<_>>` 中.

首先, 为了方便,使用 `use` 声明添加如下类型的别名.

```rust
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Db = Arc<Mutex<HashMap<String, Bytes>>>;
```

然后更新 `main` 函数来初始化是 `HashMap` 并传递 `Arc` 句柄给 `process` 函数. 使用 `Arc` 可以同时从许多任务中引用 `HashMap` , 这些
`HashMap` 也可能在许多线程上运行. 在整个Tokio中, 术语 **Handle** (这里译为 **句柄**)用于引用提供对某些共享状态访问的值.

```rust
use tokio::net::TcpListener;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    let mut listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    println!("Listening");

    let db = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // Clone the handle to the hash map.
        let db = db.clone();

        println!("Accepted");
        process(socket, db).await;
    }
}
```

注意, 这里使用的是 `std::sync::Mutex` 来保护 `HashMap` 并不是 `tokio::sync::Mutex` . 一个常见的错误是在异步代码中无条件的使用 
`tokio::sync::Mutex` .  异步的 mutex 是一种通过调用 `.await` 来锁定的互斥锁.

同步的mutex会等待获取这把锁时阻塞当前线程. 反过来, 将阻止其它任务的处理. 然而, 换成 `tokio::sync::Mutex` 通常无济于事, 是因为异步mutex
在内部使用同步互斥锁. 根据经验, 只要锁竞争保持在低水平且在对 `.await` 的调用中不持有锁, 则可以在异步代码中使用同步互斥锁. 另外可以考虑使用
`parking_log::Mutex` 作为 `std::sync::Mutex` 更快的替代方案.

## 更新 `process()` (Update `process()`)
`process()` 函数不再初始化一个 `HashMap` . 取而代之的是, 它将 `HashMap` 的共享句柄作为一个参数传进去. 在使用它之前还需要锁定 `HashMap` . 

```rust
use tokio::net::TcpStream;
use mini_redis::{connection, Frame};

async fn process(socket: TcpStream, db: Db) {
    use mini_redis::Command::{self, Get, Set};

    // 通过 mini-redis 提供 connection , 用来处理解析 socket中的帧
    let mut connection = connection::new(socket);

    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                let mut db = db.lock().unwrap();
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }           
            Get(cmd) => {
                let db = db.lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    Frame::Bulk(value.clone())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplemented {:?}", cmd),
        };

        // 写回响应到客户端
        connection.write_frame(&response).await.unwrap();
    }
}
```

## 任务,线程与竞争(Tasks, threads, and contention)
当竞争不激烈(很小)的时候, 使用阻塞的互斥锁来保护关键的部分是一种可接受的策略. 当去竞争锁时,执行任务的线程必须阻塞等待互斥锁. 这不仅将阻塞
当前任务, 还将阻塞当前线程上调度的所有其它任务.

默认情况下, Tokio运行时使用多线程的调度器. 任务可以被任何一个运行时管理的线程调度. 如果有大量的任务被调度去执行且它们都需要访问互斥锁,
这个时候就存在锁竞争. 反过来说, 如果使用 `basic_scheduler` 则互斥锁不会被竞争.

`basic_scheduler` 是一个轻量级, 单线程, 可选的运行时, 参考: [runtime option](https://docs.rs/tokio/0.2/tokio/runtime/struct.Builder.html#method.basic_scheduler) .
当仅需要产生一些任务并打开少数socket时,这是一个不错的选择. 比如说, 当提供一个同步API桥接在一个异步客户端库顶部时, 此选项很好用. 

如果在同步互斥锁上的竞争有问题时, 最好的解决办法就是少量切换到Tokio的互斥锁. 相反要考虑的选项是:
* 切换到专用任务来管理状态和使用消息传递机制.
* 分割互斥锁(译者注: 类似分段锁机制).
* 重构代码避免互斥锁.

在我们的案例中, 每一个 _key_ 都是独立的, 所以互斥锁可以很好的工作. 因此,我们将用 `N` 个不同的实例, 而不是使用单个 `Mutex<HashMap<_,_>` 实例.

```rust
type ShardedDb = Arc<Vec<Mutex<HashMap<String, Vec<u8>>>>>;
```

然后, 根据任何给定的键查找到值是两步过程. 首先, key 用来识别它是哪一部分. 然后, 在 `HashMap` 中查找key的值.

```rust
let shard = db[hash(key) % db.len()].lock().unwrap();
shard.insert(key, value);
```

(译者注: 这种分段锁思想与jdk1.8之前的ConcurrentHashMap底层实现一样).

[dashmap](https://docs.rs/dashmap) 包提供了分段hash map 实现. 

## 通过 `.await` 来持有一个 `MutexGuard` (Holding a `MutexGuard` across an `.await` )
你可能写像下面这样的代码:

```rust
use std::sync::Mutex;

async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock = mutex.lock().unwarp();
    *lock += 1;
    do_something_async().await;
} // lock 在这里超出作用域范围
```

当你尝试生成调用此函数的内容时, 会遇到以下的错误消息:

```text
error: future cannot be sent between threads safely
   --> src/lib.rs:13:5
    |
13  |     tokio::spawn(async move {
    |     ^^^^^^^^^^^^ future created by async block is not `Send`
    |
   ::: /playground/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-0.2.21/src/task/spawn.rs:127:21
    |
127 |         T: Future + Send + 'static,
    |                     ---- required by this bound in `tokio::task::spawn::spawn`
    |
    = help: within `impl std::future::Future`, the trait `std::marker::Send` is not implemented for `std::sync::MutexGuard<'_, i32>`
note: future is not `Send` as this value is used across an await
   --> src/lib.rs:7:5
    |
4   |     let mut lock = mutex.lock().unwrap();
    |         -------- has type `std::sync::MutexGuard<'_, i32>` which is not `Send`
...
7   |     do_something_async().await;
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^ await occurs here, with `mut lock` maybe used later
8   | }
    | - `mut lock` is later dropped here
```

发生这的原因是, `std::sync::MutexGuard` 类型不是 `Send` . 这意味着你不能够发送一个互斥锁到另外一个线程中, 并且会发生错误, 因为Tokio
运行时可以在每个 `.await` 的线程之间移动任务. 为了避免这种情况, 你需要重组代码来使互斥锁的析构函数在 `.await` 之前运行.

```rust
use std::sync::Mutex;
// 这是可以的!
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    {
        let mut lock = mutex.lock().unwrap();
        *lock += 1;
    }// lock 在这里超出作用域范围
    do_something_async().await;
}
```

注意下面这种无法工作:

```rust
use std::sync::Mutex;

// 这也会失败
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock = mutex.lock().unwrap();
    *lock += 1;
    drop(lock);

    do_something_async().await;
}
```

这是因为当前编译器仅根据作用域范围信息来计算一个future是否为 `Send` . 希望将来对编译器更新后会支持显式的 drop掉它, 但目前而言, 你必须显式
的使用作用域的方式.

注意这里讨论的错误也在 [Send边界](Spawning.md#Send 边界) 这里有讨论.

你不应该去尝试,通过不需要一个 `Send` 的方式来产生一个任务来规避这个问题, 是因为当任务持有锁的时候如果Tokio通过 `.await` 暂停任务, 那么可能
在同一线程上一些其它的任务可能被调度执行, 并且这些其它的任务可能也会尝试锁住互斥锁, 这将导致死锁, 因为等待锁定的互斥锁的任务将阻止持有互斥锁
的任务释放锁.

我们将在下面讨论一些解决错误消息的方法:

## 重构代码, 以免在 `.await` 中持有锁(Restructure you code to not hold the lock across an `.await` )
我们已经在上面的片段中看到了一个例子, 但这里也有更加强大的方法可能做到这一点. 比如, 你可以包装互斥锁(mutex)到一个结构体(struct)中, 且
只能将互斥锁在结构体的异步方法中锁定.

```rust
use std::sync::Mutex;

struct CanIncrement {
    mutex: Mutex<i32>,
}
impl CanIncrement {
    // 这个函数没有标识为异步函数
    fn increment(&self) {
        let mut lock = self.mutex.lock().unwrap();
        *lock += 1;
    }
}

async fn increment_and_do_stuff(can_incr: &CanIncrement) {
    can_incr.increment();
    do_something_async().await;
}
```

这种模式保证你不会遇到 `Send` 类型的错误, 是因为在异步函数的任何地方都不会出现互斥保护.

## 产生一个任务来管理状态并使用消息传递机制对其进行操作(Spawn a task to manage the state and use message passing to operate on it)
这是本章节开头提到了第二种方案, 并且当在 I/O 资源中共享资源时经常使用到. 下一章会展示更多相关的细节.

## 使用Tokio的异步互斥锁(Use Tokio's asynchronous mutex)
也可以使用 Tokio 提供的 `tokio::sync::Mutex` 类型. Tokio互斥锁主要的功能是可以在 `.await` 中持有它, 而不会产生任何问题. 也就是说, 
异步互斥锁比普通互斥锁使用起来更加昂贵, 一般最好是使用其它两种方法中的一种.

```rust
use tokio::sync::Mutex; // 注意这里是使用的 tokio的 Mutex

// 这段代码是能编译的
// (但是这种情况下选择重构代码会更好)
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock = mutex.lock().await;
    *lock += 1;

    do_something_async().await;
} // lock 在这里超出范围
```

&larr; [Spawning](Spawning.md)

&rarr; [通道](Channels.md)
