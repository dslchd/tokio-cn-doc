## I/O

在Tokio中的I/O操作几乎与`std`的相同, 但是(Tokio中的I/O操作是)是异步的. 这里有关于读( [AsyncRead](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncRead.html) )和写( [AsyncWrite](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncWrite.html) )的trait. 特定的类型适当的实现了这些trait, 比如:( [TcpStream](https://docs.rs/tokio/0.2/tokio/net/struct.TcpStream.html), [File](https://docs.rs/tokio/0.2/tokio/fs/struct.File.html), [Stdout](https://docs.rs/tokio/0.2/tokio/io/struct.Stdout.html) ) `AsyncRead` 与 `AsyncWrite` 也可以通过许多数据结构来实现, 例如, `Vec<u8>` 和 `&[u8]` . 这允许在需要一个reader与writer的地方使用字节数组.

在本章节页中, 会介绍使用Tokio来进行基本的I/O读写操作过程, 并通过一些示例进行介绍. 在下一页中我们将获得更多的关于I/O操作的高级示例.

## `异步读` 和 `异步写` (`AsyncRead` and `AsyncWrite`)
这两个trait为异步读和写入字节流提供了便利性. 这两个trait中的方法通常不能直接的调用, 就好像你不能从`Future` trait中手动的调用 `poll` 方法一样.
取而代之是, 你将通过 `AsyncReadExt` 与 `AsyncWriteExt` 提供的实用程序方法来使用它.

让我们简单的看看其中的几个方法. 所有的方法都是异步的且必须使用 `.await`.

### `async fn read()`

[AsyncReadExt::read](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncReadExt.html#method.read) 提供了一个异步的用来读取数据到缓冲区中的方法,并返回读取的字节数.

**注意** : 当 `read()` 返回 `Ok(0)` 时, 这表明流已被关闭了. 对 `read()` 的任何其它的调用将立即返回`Ok(0)`完成. 对于 [TcpStream](https://docs.rs/tokio/0.2/tokio/net/struct.TcpStream.html) 实例, 这表明socket的读取部分已经关闭.

```rust
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut f = File::open("foo.txt").await?;
    let mut buffer = [0; 10];

    // 读取10个字节
    let n = f.read(&mut buffer[..]).await?;

    println!("The bytes: {:?}", &buffer[..n]);
    Ok(())
}
```

### `async fn read_to_end()`

 [AsyncReadExt::read_to_end](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncReadExt.html#method.read_to_end) 
 从流中读取所有的字节直到遇到 EOF.

 ```rust
 use tokio::io::{self, AsyncReadExt};
 use tokio::fs::File;
 
 #[tokio::main]
 async fn main() -> io::Result<()> {
     let mut f = File::open("foo.txt").await?;
     let mut buffer = Vec::new();
 
     // 读取整个文件
     f.read_to_end(&mut buffer).await?;
     Ok(())
 }
 ```

### `async fn write()`

[AsyncWriteExt::write](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncWriteExt.html#method.write) 将缓冲区中的数据写入到writer
并返回写入的字节数.

```rust
use tokio::io::{self, AsyncWriteExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut file = File::create("foo.txt").await?;

    // 写入字字节符串的一些前缀, 但不一定是全部
    let n = file.write(b"some bytes").await?;

    println!("Write the first {} bytes of 'some bytes'.", n);
    Ok(())
}
```

### `async fn write_all()`

[AsyncWriteExt::write_all](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncWriteExt.html#method.write_all) 将整个缓存区写入到writer.

```rust
use tokio::io::{self, AsyncWriteExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()>{
    let mut buffer = File::create("foo.txt").await?;
    
    buffer.write_all(b"some bytes").await?;
    Ok(())
}
```
 这两个trait都包含了其它有用的方法. 有关完整的列表, 请参考API文档.

 ## 辅助函数(Helper functions)
 另外, 与 `std` 包中一样, `tokio::io`模块也包含了一些有用的实用函数和用于处理标准输入,输出,错误的API. [standard input](https://docs.rs/tokio/0.2/tokio/io/fn.stdin.html),
 [standard output](https://docs.rs/tokio/0.2/tokio/io/fn.stdout.html), [standard error](https://docs.rs/tokio/0.2/tokio/io/fn.stderr.html) .
 比如, `tokio::io::copy` 可以异步将reader中的全部内容复制到writer中去.

 ```rust
 use tokio::fs::File;
 use tokio::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut reader: &[u8] = b"hello";
    let mut file = File::create("foo.txt").await?;
    
    io::copy(&mut reader, &mut file).await?;
    Ok(())
}
 ```

注意, 这利用了字节数组也实现了 `AsyncRead` 这一特点.

## 回声服务器(Echo server)
让我们练习一些异步I/O. 我们将编写一个回声服务.

此回声服务绑定一个 `TcpListener` 且在一个循环中接收入站链接. 对于每个链接将从socket中读取数据并将数据立即写回到socket中.
客户端发送数据到服务端并接收回同样的返回.

我们将使用略微不同的策略来两次实现echo服务.

### 使用 `io::copy()` (Using `io::copy()`)
首先,我们将使用`io::copy()` 实现echo的逻辑部分.

这是一个TCP服务,需要一个accept循环. 产生一个任务来处理每一个被接收的Socket链接.

```rust
use tokio::io;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut listener = TcpListener::bind("127.0.0.1:6124").await.unwrap();
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            // 这里Copy数据
        });
    }
}
```

和上面看到的一样, 这个实用函数需要一个reader和一个writer并从它们中的一个复制数据到另外一个中去. 然而, 我们只有一个`TcpStream`.
该单一值同时实现了`AsyncReader`和`AsyncWrite`. 因为`io::copy`的reader和writer都需要`&mut`, 所有socket不能同时用于两个参数.

```rust
// 这样无法编译
io::copy(&mut socket, &mut socket).await?;
```

### 拆分reader与writer(Splitting a reader + writer)
为了解决这个问题, 我们必须分割socket到一个reader处理器与一个writer处理器中去. 拆分一个reader/writer组合最佳的方法依赖一个特定的类型.
任何reader+writer类型都能被 `io::split` 工具拆分. 这个函数传入单个值,并返回单独的reader和writer处理器. 这两个处理器可以单独的使用,
包括从不同的任务中.

比如, echo客户端能像下面这样处理并发读与写:

```rust
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = TcpStream::connect("127.0.0.1:6124").await?;
    let (mut rd, mut wr) = io::split(socket);

    // 在后台写入数据
    let write_task = tokio::spawn(async move {
        wr.write_all(b"hello\r\n").await?;
        wr.write_all(b"world\r\n").await?;

        // 有时候Rust的推导需要一点帮助
        Ok::<_, io::Error>(())
    });

    let mut buf = vec![0; 128];

    loop {
        let n = rd.read(&mut buf).await?;

        if n == 0 {
            break;
        }

        println!("GOT {:?}", &buf[..n]);
    }

    Ok(())
}
```

因为 `io::split` 支持任意实现了`AsyncRead+AsyncWrite` 类型的值且返回独立的处理器, `io::split`内部使用了`Arc`与`Mutex`. 使用`TcpStream`可以避免这种开销, `TcpStream` 提供了两个专门的拆分函数.

[TcpStream::split](https://docs.rs/tokio/0.2/tokio/net/struct.TcpStream.html#method.split) 引用流并返回一个reader和writer的处理器.因为使用了引用,所以两个处理器都必须保持与调用`split()`相同的任务一致. 这个特殊的`split`是零成本的. 这里不需要`Arc`或者`Mutex`. `TcpStream`也提供了一个 [into_split](https://docs.rs/tokio/0.2/tokio/net/struct.TcpStream.html#method.into_split) 功能,此功能支持仅需要`Arc`就能跨任务移动处理器.

因为`io::copy()`在属于`TcpStream`的同一个任务上被调用,所以我们可以使用 [TcpStream::split](https://docs.rs/tokio/0.2/tokio/net/struct.TcpStream.html#method.split).处理echo逻辑服务的任务变为:

```rust
tokio::spawn(async move{
    let(mut rd, mut wr) = socket.split();
    
    if io::copy(&mut rd, &mut wr).await.is_err() {
        eprintln!("failed to copy");    
    }
});
```

你可以 [这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/io/src/echo-server.rs) 找到完整的代码.

### 手动复制(Manual copying)
现在让我们来看看如何通过手动的复制数据来编写echo服务器. 为了做到这一点,我们使用 [AsyncReadExt::read](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncReadExt.html#method.read)
和 [AsyncWriteExt::write_all](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncWriteExt.html#method.write_all) .

完整的echo服务像下面这样:

```rust
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut listener = TcpListener::bind("127.0.0.1:6124").await.unwrap();

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = vec![0; 1024];

            loop {
                match socket.read(&mut buf).await {
                    // 返回 Ok(0) 值标识远程链接已关闭.
                    Ok(0) => return,
                    Ok(n) => {
                        // 复制数据到socket中
                        if socket.write_all(&buf[..n]).await.is_err() {
                            // 未期待的socket错误, 这里我们不做什么,因此停止处理.
                            return;
                        }
                    }
                    Err(_) => {
                         // 未期待的socket错误, 这里我们不做什么,因此停止处理.
                        return;
                    }
                }
            }
        });
    }
}
```

让我们分解一下上面的过程. 首先, 由于使用了`AsyncRead`和`AsyncWrite`, 其扩展的trait必须要被引入到范围内.

```rust
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
```

(译者注: 上面有说过,我们仅能使用其扩展的trait)

### 分配一个缓冲区(Allocating a buffer)
有种策略是从socket中读取一些数据到buffer(缓冲区)中,然后将缓冲区的内容写回到socket中去.

```rust
let mut buffer = vec![0;1024];
```

要明确的避免栈缓冲区. 回想一下 [之前的](./Spawning.md) (中的Send边界), 所有通过对`.await`调用存活的任务数据都必须由任务本身存储.
在这种情况下,将在`.await`的调用中使用`buf`. 所有的任务数据都被存储在一个分配中. 你可以将其看作一个枚举, 其中每个变量体都是为特定调用
`.await`而需要存储的数据.

如果buffer由栈数组来表示, 那么每一个接受socket产生的任务的内部结构可能类似于:

```rust
struct Task {
    // 内部的任务字段
    task: enum {
        AwaitingRead {
            socket: TcpStream,
            buf: [BufferType],
        },
        AwaitingWriteAll {
            socket: TcpStream,
            buf: [BufferType],
        }

    }
}
```

如果栈数组被使用来作来buffer的类型, 它将以 _内联_ 的方式存储在任务结构中. 这将使用任务本身的结构变得非常大. 另外缓冲区buffer的大小通常是
页面大小. 反过来,这会使用任务(Task)大小变得很臃肿: `$page-size + a-few-bytes`.

编译器对异步结块布局的优化比基本的`enum`(枚举)更加好. 实际上,变量不会像枚举那样在变体中移动. 但是,任务结构体的大小至少与最大变量一样大.

### 处理 EOF(Handling EOF)
(译者注: EOF: "end of file" 的缩写, 表示 "文字流结尾" 这种流(Stream) 可以是文件,也可以是标准输入. 一般理解为流的结束标识)

当读取TCP流的一半时关闭了, 调用`read()`会返回`Ok(0)`. 以这一点来退出循环是很重要的. 忘记以EOF标识来跳出循环是bug的常见来源方式.

```rust
loop {
    match socket.read(&mut buffer).await {
        // 返回值是 Ok(0) 标志, 表示远端已经关闭
        Ok(0) => {
            // 其它处理
        }
    }
}
```

忘记以EOF标识来跳出循环的结果就是会造成CPU 100%循环占用. 关闭socket后, `socket.read()` 会立即返回. 然后循环会一直重复下去.

完整的代码参考 [这里](https://github.com/tokio-rs/website/blob/master/tutorial-code/io/src/echo-server.rs)

&larr; [通道(Channels)](Channels.md)

&rarr; [帧(Framing)](Framing.md)