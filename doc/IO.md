## I/O

在Tokio中的I/O操作几乎与`std`的相同, 但是(Tokio中的I/O操作是)是异步的. 这里有关于读( [AsyncRead](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncRead.html) )
和写( [AsyncWrite](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncWrite.html) )的trait. 特定的类型适当的实现了这些trait, 比如:
( [TcpStream](https://docs.rs/tokio/0.2/tokio/net/struct.TcpStream.html), [File](https://docs.rs/tokio/0.2/tokio/fs/struct.File.html), [Stdout](https://docs.rs/tokio/0.2/tokio/io/struct.Stdout.html) )
`AsyncRead` 与 `AsyncWrite` 也可以通过许多数据结构来实现, 例如, `Vec<u8>` 和 `&[u8]` . 这允许在需要一个reader与writer的地方使用字节数组.

在本章节页中, 会介绍使用Tokio来进行基本的I/O读写操作过程, 并通过一些示例进行介绍. 在下一页中我们将获得更多的关于I/O操作的高级示例.

## `异步读` 和 `异步写` (`AsyncRead` and `AsyncWrite`)
这两个trait提供了异步读和写入字节流的便利性. 这两个trait中的方法通常不能直接的调用, 就好像你不能从`Future` trait中手动的调用 `poll` 方法一样.
取而代之是, 你将通过 `AsyncReadExt` 与 `AsyncWriteExt` 提供的实用程序方法来使用它.

让我们简单的看看其中的几个方法. 所有的方法都是异步的且必须使用 `.await`.

`async fn read()`

[AsyncReadExt::read](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncReadExt.html#method.read) 提供了一个异步的用来读取数据到缓存中的方法,
并返回读取的字节数.

**注意** : 当 `read()` 返回 `Ok(0)` 时, 这表明流已被关闭了. 对 `read()` 的任何其它的调用将立即返回`Ok(0)`完成. 
对于 [TcpStream](https://docs.rs/tokio/0.2/tokio/net/struct.TcpStream.html) 实例, 这表明socket的读取部分已经关闭.

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

`async fn read_to_end()`
 
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

`async fn write()`

[AsyncWriteExt::write](https://docs.rs/tokio/0.2/tokio/io/trait.AsyncWriteExt.html#method.write) 将缓冲区中的数据写入到writer
并返回写入的字节数.

```rust
use tokio::io::{self, AsyncWriteExt};
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut file = File::create("foo.txt").await?;

    // Writes some prefix of the byte string, but not necessarily all of it.
    let n = file.write(b"some bytes").await?;

    println!("Wrote the first {} bytes of 'some bytes'.", n);
    Ok(())
}
```

