## Spawning
我们将切换且开始在Redis Server上的工作.

首先, 将客户端的 `SET/GET` 代码从上一章节移至一个示例文件中. 这样的话我们可以在服务器上运行它.

```shell script
mkdir -p examples
mv src/main.rs examples/hello-redis.rs
```

## 接收套接字(Accepting sockets)
我们的Redis服务器需要做的第一件事是接受入站的TCP sockets. 这使用 `tokio::net::TcpListener` 来完成.

```text
大多数Tokio的类型名称被命名为和Rust标准库中具有等效功能的相同类型的名称一样. 在合理的情况个 Tokio 暴露了与 std 库相同的API, 只是tokio使用了
async fn .
```

一个 `TcpListener` 绑定到端口6379, 然后在loop循接受sockets. 每个socket 都经过处理然后关闭. 现在,我们将读取命令,将它打印到标准输出并返回错误.

```rust
use tokio::net::{TcpListener, TcpStream};
use mini_redis::{connection, Frame};

#[tokio::main]
async fn main() {
    // 绑定监听器到一个地址
    let mut listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        // 第二项包含新链接的IP与端口
        let (socket, _) = listener.accept().await.unwrap();
        process(socket).await;
    }
}

async fn process(socket: TcpStream) {
    // "链接" 可以让我们通过字节流 读/写 redis的 **帧**. "链接" 类型被 mini-redis 定义.
    let mut connection = connection::new(socket);

    if let Some(frame) = connection.read_frame().await.unwrap() {
        println!("GOT: {:?}", frame);

        // 响应一个错误
        let response = Frame::Error("unimplemented".to_string());
        connection.write_frame(&response).await.unwrap();
    }
}
```

现在运行一个这个accept loop:

```shell script
cargo run
```

在另外一个窗口中,运行 `hello-redis` 示例(上一个章节有 `SET/GET` 命令的示例):
```shell script
cargo run --example hello-redis
```

应该会输出:

```shell script
Error: "unimplemented"
```

在服务端终端会输出:

```shell script
GOT: Array([Bulk(b"set"), Bulk(b"hello"), Bulk(b"world")])
```

## 并发(Concurrency)
我们的服务有点小问题(除了仅响应错误之外). 它一次处理一个入站请求. 当一个链接被接受后, 服务器将停在accept循环块中直到响应完成写入到socket中为止.

我们希望我们的Redis服务能处理 **更多** 的并发请求. 为了做到这一点,我们必须添加一些并发.

```text
并发与并行不是同一件事. 如果你在两个任务之间交替执行, 那么你将同时执行两个任务, 但不能并行执行.(译者注: 这种情况属于并发,不是并行)
为了使它具有并行性, 你需要两个人, 每个人专门负责每个任务.(译者注: 同时并行的执行,而不是交替). 

使用tokio的优点是异步代码可以让你同时处理许多并发任务, 而不必使用普通线程并行处理它们. 事实上, Tokio可以在单个线程上并发处理许多任务.
```

为了同时处理链接,将为每一个入站的新链接产生一个新任务. 这个链接在此任务中处理.

accept 循环会变为:
```rust
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let mut listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // 为每一个入站socket链接产生一个新任务. 此socket链接被移动到这个新任务中且在里面处理.
        tokio::spawn(async move {
            process(socket).await;
        });
    }
}
```

## 任务(Tasks)
一个Tokio的任务(task)是一个异步的绿色线程. 它们通过 `async` 块 `tokio::spawn` 来创建. `tokio::spawn` 函数返回一个　`JoinHandle`
,　调用者可以使用该　`JoinHandle` 与生成的任务进行交互. `async` 块可以有一个返回值. 调用方可以在 `JoinHandle` 上使用　`.await` 获取返回值.

比如:
```rust
#[tokio::main]
async fn main() {
    let handle = tokio::spawn(async {
        // 做一些异步的工作
        "return value"
    });

    // 作一些其它的工作

    let out = handle.await.unwrap();
    println!("GOT {}", out);
}
```
在 `JoinHandle` 等待返回一个　`Result` . 当任务在处理期间遇到一个错误时, `JoinHandle` 会返回一个 `Err`. 这种情况发生在, 当任务出现 panics 或者
任务在运行期间被关闭而强制取消时.

任务是由调度器管理的执行单元. 产生的任务会提交给Tokio的调度器, 调度器可以确保在有工作要做时执行任务. 产生的任务可以在与产生它的同一线程上执行,
也可以在不同的运行时线程上执行. 任务产生后也可以在不同的线程之间移动.

## 静态边界(`'static bound`)
通过　`tokio::spawn` 产生的任务必须是　`'static` 的. 产生(Spawned)的表达式不能借用任何数据.

有一种普遍的误解是, "静态"意味着"永久存活"("being static" means "lives forever"),　然而情况不并是这样. 如果仅仅因为值是　`'static` 
的话并不意味着存在内存泄露. 有关这点你可以在[Common Rust Lifetime Misconceptions](https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md#2-if-t-static-then-t-must-be-valid-for-the-entire-program)
了解更多.

例如, 下面的示例将不能被编译:
```rust
use tokio::task;

#[tokio::main]
async fn main() {
    let v = vec![1,2,3];
    
    task::spawn(async {
        println!("Here's a vec: {:?}", v);
    });
}
```
尝试去编译会产生如下错误结果:
```text
error[E0373]: async block may outlive the current function, but
              it borrows `v`, which is owned by the current function
 --> src/main.rs:7:23
  |
7 |       task::spawn(async {
  |  _______________________^
8 | |         println!("Here's a vec: {:?}", v);
  | |                                        - `v` is borrowed here
9 | |     });
  | |_____^ may outlive borrowed value `v`
  |
note: function requires argument type to outlive `'static`
 --> src/main.rs:7:17
  |
7 |       task::spawn(async {
  |  _________________^
8 | |         println!("Here's a vector: {:?}", v);
9 | |     });
  | |_____^
help: to force the async block to take ownership of `v` (and any other
      referenced variables), use the `move` keyword
  |
7 |     task::spawn(async move {
8 |         println!("Here's a vec: {:?}", v);
9 |     });
  |
```
为发生这种情况是因为, 默认情况下, 变量是不能被移动到一个异步块中的. `v` 集合仍然被　`main` 函数所有. `println!` 行借用了 `v` .  
rust的编译器能够帮助解释这一点, 甚至可以提出修改的建议! 修改第７行为 `task::spawn(async move {` 这将指示编译器将移动　`v` 到产生的任务中去.
现在任务拥有它自己的所有数据并使其为 `'static` .

如果必须同时从多个并发任务中访问单个数据, 则必须使用共享同步原语, 例如 `Arc` .

## Send 边界(`Send` bound)
通过 `tokio::spawn` 产生的任务必须实现　`Send` . 这允许Tokio运行时在任务使用 `.await` 挂起时可以在不同的线程之间移动他们.

当通过　`.await`　调用中保存的所有数据都为　`Send` 时, 任务就是一个　`Send` . 这点有些微妙. 当　`.await` 被调用时任务会返回到调度器.
下一次任务被执行会从上一次的出让点(point it last yielded)继续.(译者注: 从哪个地方出让并返回到调度器,下一次任务执行时就从那个点恢复).
若要进行这样的工作, 该任务必须保存　`.await`　之后使用的所有状态. 如果这个状态是　`Send` , 比如, 能在不同线程中移动, 则任务本身就可以跨
线程移动. 反过来, 如果状态不是 `Send` 的, 那么任务本身也就不能跨线程移动.

例如, 这种有效:
```rust
use tokio::task::yield_now;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        // 在 .await 之前作用域强制　rc drop了
        {
            let rc = Rc::new("hello");
            println!("{}", rc);
        }

        // rc 不再使用. 当任务返回到调度器后,  rc 不能再持续下去
        yield_now().await;
    });
}
```

这一种情况却不行:
```rust
use tokio::task::yield_now;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        let rc = Rc::new("hello");

        // rs 在　.await后继续使用, 它必须持久化到 task　的　状态中才行
        yield_now().await;

        println!("{}", rc);
    });
}
```

尝试编译上面的代码片段,会有如下结果:
```text
error: future cannot be sent between threads safely
   --> src/main.rs:6:5
    |
6   |     tokio::spawn(async {
    |     ^^^^^^^^^^^^ future created by async block is not `Send`
    | 
   ::: [..]spawn.rs:127:21
    |
127 |         T: Future + Send + 'static,
    |                     ---- required by this bound in
    |                          `tokio::task::spawn::spawn`
    |
    = help: within `impl std::future::Future`, the trait
    |       `std::marker::Send` is not  implemented for
    |       `std::rc::Rc<&str>`
note: future is not `Send` as this value is used across an await
   --> src/main.rs:10:9
    |
7   |         let rc = Rc::new("hello");
    |             -- has type `std::rc::Rc<&str>` which is not `Send`
...
10  |         yield_now().await;
    |         ^^^^^^^^^^^^^^^^^ await occurs here, with `rc` maybe
    |                           used later
11  |         println!("{}", rc);
12  |     });
    |     - `rc` is later dropped here
```
在 [下一章](SharedState.md) 中, 我们将更深入的讨论这种错误的特殊情况.

## 存储值(Store values)
现在我们将实现一个　`process` 函数来处理传入的命令. 我们使用一个　`HashMap` 来存值. `SET` 指令将插入到　`HashMap`　中而　`GET` 指令
将它们从 `HashMap` 中加载出来. 另外, 我们将使用一个循环来接受每个链接的多个指令. 

```rust
use tokio::net::TcpStream;
use mini_redis::{connection, Frame};

async fn process(socket: TcpStream) {
    use mini_redis::Command::{self, Get, Set};
    use std::collections::HashMap;

    // 存储数据的HashMap
    let mut db = HashMap::new();

    // 通过 mini-redis 提供的链接, 可以处理来自socket中的　帧
    let mut connection = connection::new(socket);

    // 使用　read_frame 来接收一个来自　链接中的　命令
    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                if let Some(value) = db.get(cmd.key()) {
                    Frame::Bulk(value.clone())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplemented {:?}", cmd),
        };

        // 写入响应到客户端
        connection.write_frame(&response).await.unwrap();
    }
}

```
现在启动这个服务:

```shell script
cargo run
```

并且在另外一个窗口中运行　`hello-redis` 示例:

```shell script
cargo run --example hello-redis
```

现在得到了如下的输出:
```text
got value from the server; success=Some(b"world")
```

现在我们可以获取和设置一个值, 但是这里还有一个问题: 值不能够在链接中共享. 如果另外一个socket链接尝试通过　`GET` 得到键　`hello` 的值,
这将不会找任何东西.

你可以在 [这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/spawning/src/main.rs) 找到完整的代码.

在下一章节中,我们将为所有的sockets链接实现持久化数据.

&larr; [你好Tokio](HelloTokio.md)

&rarr; [共享状态](SharedState.md)
