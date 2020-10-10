## 你好 Tokio(Hello Tokio)
我们将通过编写一个非常基础的Tokio应用来开始. 它可以连接到Mini-Redis服务, 并设置键 `hello` 的值为 `world` . 然后它可以读取这个键, 这将
使用Mini-Redis客户端完成.

## 实现代码(The code)
### 生成一个新的包
让我们创建一个新的Rust app:
```shell script
cargo new my-redis
cd my-redis
```

### 添加依赖(Add dependencies)
下一步, 打开 `Cargo.toml` 并在 `[dependencies]` 下添加如下依赖:
```shell script
tokio = {version = "0.2", features = ["full"]}
mini-redis = "0.2"
```

### 编写代码(Write the code)
然后, 打开 `main.rs` 并使用如下内容替换:
```rust
use mini_redis::{client, Result};

#[tokio::main]
pub async fn main() -> Result<()> {
    // 打开链接到mini-redis的链接
    let mut client = client::connect("127.0.0.1:6379").await?;

    // 设置 "hello" 键的值为 "world"
    client.set("hello", "world".into()).await?;

    // 获取"hello"的值
    let result = client.get("hello").await?;

    println!("got value from the server; result={:?}", result);

    Ok(())
}
```
确保 Mini-Redis服务正在运行. 在另外一个终端窗口中运行:
```shell script
mini-redis-server
```
现在, 运行 `my-redis` 应用:
```shell script
cargo run 
got value from the server; result=Some(b"world")
```
成功了!

你可以在 [这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/hello-tokio/src/main.rs) 找到完整的代码.

## 过程分解(Breaking it down)
让我们花一点时间来回顾一下刚刚上面做的事情. 这里没有太多的代码, 但是却发生了很多事.
```rust
let mut client = client::connect("127.0.0.1:6379").await?;
```
`client::connect` 函数功能由 `mini-redis` 包提供. 它通过一个指定的远程地址异步的建立一个TCP链接. 一旦链接被建立, 一个 `client` 处理器就会被返回.
即使操作是异步的,但我们写的代码看起来是同步的. 操作是异步的唯一标识是 `.await` 操作符.

### 什么是异步编程?(what is asynchronous programming?)
大多数计算机程序的执行顺序都是按程序编写的顺序来执行的. 第一行代码先执行,然后是下一行,这样一直下去. 对于同步编程, 当程序遇到不能立即完成的操作时,
它将被阻塞直到该操作完成为止. 比方说, 建立TCP链接需要对等方通过网络进行交换, 这一过程可能需要相当长的时间, 期间, 线程是阻塞的.

对于异步编程不能立即完成的操作会被挂起到后台. 当前线程不会被阻塞, 并且能继续运行其它的事. 一旦操作完成, 任务将从中断处继续处理. 我们前面的示例
只有一个任务, 因此在挂起时什么也没发生, 但是通常异步程序有很多这样的任务.

尽管异步编程可以使得应用程序更快, 但它通常也会导致程序复杂的多. 一旦异步操作完成, 就需要程序员跟踪恢复工作所需的所有状态. 从历史角度来看, 这是一个非常
乏味且容错出错的任务.

### 编译时绿色线程(Compile-time green-threading)
Rust使用被叫作 `async/await` 的feature实现异步编程. 执行异步操作的函数用 `async` 关键字来标记. 在我们的示例当中, `connect` 函数被定义像下面这样:
```rust
use mini_redis::Result;
use mini_redis::client::Client;
use tokio::net::ToSocketAddress;

pub async fn connect<T: ToSocketAddress>(addr: T) -> Result<Client> {
    // ...
}
```
使用 `async fn` 的定义看起来像同步函数一样,  但是操作是异步的. Rust在编译时将转换 `async fn` 为异步操作. 任何在 `async fn` 中的 `.await`
调用都会将控制权返回给线程. 在后台进行操作时, 线程可能会做其它的工作.

```text
尽管其它语言也实现了 `async/await`, 但是Rust的实现比较独特. 主要是Rust的异步操作是惰性(lazy)的. 结果就是导致与其它语言不同的运行时语义.
```

如果这样说还不太明白其意义,请不用担心. 我们将在本指南中进一步探讨 `async/await` .

### 使用 `async/await`(Using `async/await` )
异步函数的调用与其它Rust函数一样. 但是调用这些函数不会导致函数体执行(译者注: 即是一种声明不会立即执行). 而是调用这些函数返回表示操作的值.
从概念上讲,这类似于零参数闭包. 为了实际的去运行这些操作, 你应该使用 `.await` 操作符来返回值.

通过下面的程序举例:
```rust
async fn say_world() {
    println!("world");
}

#[tokio::main]
async fn main() {
    // 调用 say_world函数没有立即执行 say_world() 函数体
    let op = say_world();

    // 这里会首先打印
    println!("hello");

    // 调用 .await 操作才会执行say_world
    op.await;
}
```
输出:
```text
hello
world
```
`async fn` 返回值是一个实现了 `Future` trait的异步类型.

### 异步 `main` 函数(Async `main` function)
main 函数用来启动应用, 它与大多数Rust其它包中的函数不同:

* 1. 它是一个 `async fn` .
* 2. 它使用 `#[tokio::main]` 注解.

我们要进入一个异步上下文时,一个 `async fn` 被使用. 但是异步函数必须由一个运行时 [runtime](https://docs.rs/tokio/0.2/tokio/runtime/index.html) 来执行.
运行时包含异步任务调度器(scheduler), 提供I/O事件, 计时器(timers)等等. 运行时不会自动开始,因此需要main函数来启动它.

`#[tokio::main]` 函数宏. 它将 `async fn main()` 转换为一个初始化一个运行时实例且执行异步main函数的 同步 `fn main()`. 

比如说下面的示例:
```rust
#[tokio::main]
async fn main() {
    println!("hello");
}
```
转换后的结果:
```rust
fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("hello");
    })
}
```
tokio运行时的细节将在后面介绍.

### Cargo 特性(Cargo features)
当本教程中的Tokio依赖,启用了 `full` 特性时:
```toml
tokio = {version = "0.2", features = ["full"]}
```
Tokio有很多功能(TCP,UDP,Unix 套接字, 定时器(timers), 同步工具(sync utilities),多种调度器类型(multiple scheduler types), 等等).
不是所有的应用都需要所有的功能. 当我们尝试去优化编译时间或者应用占用空间时, 应用程序可以去选择仅仅它需要使用的特性.

比如现在我们在tokio的依赖中使用了 "full" 的特性.

[指南](Introduction.md) <-----------------------------------------------------------------------> [Spawning](Spawning.md)

<script type="math/tex; mode=display" id="MathJax-Element-11433">符号</script>
