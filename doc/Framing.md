## 帧(Framing)
现在,我们将应用刚刚学到的的I/O知识,并以此来实现Mini-Redis的帧层. 形成帧是获取字节流并将其转换为帧流的过程. 帧是两个对等体之间传输数据的单位.
Redis协议帧像下面这样:

```rust
use bytes::Bytes;

enum Frame {
    Simple(String),
    Error(String),
    Integer(u64),
    Bulk(Bytes),
    Null,
    Array(Vec<Frame>),
}
```

注意帧仅由没有任何语义的数据组成. 命令的解析和实现发生在更高的层级.

比如说, HTTP的帧可能看起来像下面这样:

```rust
enum HttpFrame {
    RequestHead {
        method: Method,
        uri: Uri,
        version: Version,
        headers: HeaderMap,
    },
    ResponseHead {
        status: StatusCode,
        version: Version,
        headers: HeaderMap,
    },
    BodyChunk {
        chunk: Bytes,
    },
}
```

为了去实现Mini-Redis的帧, 我们将实现一个`connection`结构体, 该结构体包装了一个`TcpStream`并读取/写入`mini_redis::Frame`的值.

```rust
use tokio::net::TcpStream;
use mini_redis::{Frame, Result};

struct connection {
    stream: TcpStream,
    // ... 其它属性字段
}

impl connection {
    /// 从Connection中读取一个帧
    /// 
    /// 如果 EOF 到达则返回 None
    pub async fn read_frame(&mut self)
        -> Result<Option<Frame>>
    {
        // 在这里实现
    }

    /// 写入一个帧到链接Connection中
    pub async fn write_frame(&mut self, frame: &Frame)
        -> Result<()>
    {
        // 在这里实现
    }
}
```

你能在[这里](https://redis.io/topics/protocol) 找到完整的Redis协议细节. 完整的 `connection` 代码在 [这里](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/connection.rs) .

## 缓冲区读取(Buffered reads)
`read_frame` 方法在返回之前会等待接收整个帧. 单次调用`TcpStream::read()`方法可能会返回任意数量的数据. 它可能包含一个完整的帧,一部分帧,或者多个帧.
如果接收到部分帧,则会被缓存,并从socket套接字中读取更多的数据. 如果接收到多个帧, 则返回第一个帧,并缓冲其它的数据,直到下一次调用`read_frame`为止.

为了实现这一点, `connection`需要一个读取缓冲区字段. 数据从socket中读取到读缓冲区(read buffer)中. 当一个帧被解析时, 相应的数据就会从缓冲区中移除.

我们将使用 [BytesMut](https://docs.rs/bytes/0.5/bytes/struct.BytesMut.html) 作为缓冲区(buffer)的类型. 这是一个 [Bytes](https://docs.rs/bytes/) 的可变版本.

```rust
use bytes::BytesMut;
use tokio::net::TcpStream;

pub struct connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl connection {
    pub fn new(stream: TcpStream) -> connection {
        connection {
            stream,
            // 默认分配buffer容量为4kb
            buffer: BytesMut::with_capacity(4096),
        }
    }
}
```

下一步,我们将实现 `read_frame()` 方法.

```rust
use tokio::io::AsyncReadExt;
use bytes::Buf;
use mini_redis::Result;

pub async fn read_frame(&mut self)
    -> Result<Option<Frame>>
{
    loop {
        // 尝试从buffer数据中解析一个帧. 如果buffer中有足够的数据,那么帧就返回
        if let Some(frame) = self.parse_frame()? {
            return Ok(Some(frame));
        }

        // 没有足够的数据读取到一个帧中, 那么尝试从socket中读取更多的数据
        // 如果成功了, 一定数据的字节被返回. '0' 表明到了流的末尾
        if 0 == self.stream.read_buf(&mut self.buffer).await? {
            // 远程关闭了链接,为了彻底关闭, 读缓冲区中应该没有数据了. 如果存在数据, 那说明对等方在发送帧时关闭了socket
            return if self.buffer.is_empty() {
                Ok(None)
            } else {
                Err("connection reset by peer".into())
            }
        }
    }
}
```

让我们分解一下. `read_frame` 方循环运行. 首先, `self.parse_frame()` 方法被调用. 这将尝试从`self.buffer` 中解析一个redis帧.
如果这里有足够的数据解析成一个帧, 那么就会返回给`read_frame()`调用者一个帧. 否则的话,我们将尝试从socket中读取更多的数据到缓冲区中.
读取更多数据后,再一次调用`parse_frame()`方法. 这一次, 如果已经接收到足够的数据,那么就能解析成功.

当从流(Stream)中读取时,返回值0表示不再从对等方接收数据. 如果读取缓冲区中任然有数据,则表明已经接收到部分帧,并且链接突然终止了. 这种情况是
一种错误并会返回`Err`. 

## `Buf` trait
当从流中读取时, 将调用 `read_buf`方法. 这个版本的read函数采用了一个从 [bytes](https://docs.rs/bytes/) 包中实现了 [BufMut](https://docs.rs/bytes/0.5/bytes/trait.BufMut.html) 的值.

首先,考虑如何使用`read()`实现同样的读取循环. 可以使用`Vec<u8>`来代替`BytesMut`.

```rust
use tokio::net::TcpStream;

pub struct connection {
    stream: TcpStream,
    buffer: Vec<u8>,
    cursor: usize,
}

impl connection {
    pub fn new(stream: TcpStream) -> connection {
        connection {
            stream,
            // 分配4kb的缓冲区容量
            buffer: vec![0; 4096],
            cursor: 0,
        }
    }
}
```

`connection`上的`read_frame()` 函数.

```rust
use mini_redis::{Frame, Result};

pub async fn read_frame(&mut self) -> Result<Option<Frame>>
{
    loop {
        if let Some(frame) = self.parse_frame()? {
            return Ok(Some(frame));
        }

        // 确保buffer有容量
        if self.buffer.len() == self.cursor {
            // 增长buffer
            self.buffer.resize(self.cursor * 2, 0);
        }

        // 读取到缓冲区, 跟踪读取的字节数
        let n = self.stream.read(
            &mut self.buffer[self.cursor..]).await?;

        if 0 == n {
            if self.cursor == 0 {
                return Ok(None);
            } else {
                return Err("connection reset by peer".into());
            }
        } else {
            // 更新游标
            self.cursor += n;
        }
    }
}
```

在使用字节数组进行读取时, 我们还必须保持一个游标,来跟踪已经缓冲了多少数据. 我们必须确保缓冲区的空白部分传递给 `read()`. 否则会覆盖缓冲区的数据.
如果缓冲区被填满, 我们必须增加缓冲区来继续读取. 在`parse_frame()`(但不包括)中, 我们还必须解析`self.buffer[..self.cursor]`包含的数据.

因为将字节数据与游标配对非常常见, 所以 `bytes` 包中提供了代表字节数组和游标的抽象. `Buf` trait可以被需要读取数据的类型实现. `BufMut` trait可以被
需要数据写入的类型实现. 当传递一个 `T:BufMut` 到 `read_buf()`时, 缓冲区的内部游标由`read_buf()`自动更新. 因为这一点,在我们的`read_frame`版本中,
我们不需要自己来管理自己的游标.

另外, 当使用 `Vec<u8>` 时缓冲区必须要初始化. `vec![0; 4096]` 分配一个大小为4096字节的数组并在每个位置写0. 当调整buffer的大小时,新的容量也必须要使用0
来初始化. 初始化的过程不是无消耗的. 当使用`BytesMut`和`BufMut`时,容量是**未初始化**的. `BytesMut`抽象阻止了我们读取取未初始化的内存. 这使得我们避免了
初始化的步骤.

## 解析(Parsing)
现在,让我们来看看`parse_frame()`函数. 解析的过程分两步:

1. 确保已经缓冲整个帧并找到帧结束索引.
2. 解析一个帧.

`mini-redis` 包提供给我们一个解决上面两步功能的函数:

1. [Frame::check](https://docs.rs/mini-redis/0.3/mini_redis/frame/enum.Frame.html#method.check)
2. [Frame::parse](https://docs.rs/mini-redis/0.3/mini_redis/frame/enum.Frame.html#method.parse)

我们也将重用`Buf`抽象来得到帮助. 一个`Buf` 传递到`Frame::check`中去. 当`check`函数迭代传入buffer时, 内部的游标也会前进. 当`check`
函数返回时, 缓冲区(buffer)的内部游标会指向帧的末尾.

对于`Buf`的类型,我们使用 [std::io::Cursor<&[u8]>](https://doc.rust-lang.org/stable/std/io/struct.Cursor.html)

```rust
use mini_redis::{Frame, Result};
use mini_redis::frame::Error::Incomplete;
use bytes::Buf;
use std::io::Cursor;

fn parse_frame(&mut self)
    -> Result<Option<Frame>>
{
    // 创建一个 T:Buf 类型
    let mut buf = Cursor::new(&self.buffer[..]);

    // 检查是否为一个完整可用的帧
    match Frame::check(&mut buf) {
        Ok(_) => {
            // 得到帧的字节长度
            let len = buf.position() as usize;
d
            // 调用parse来重围内部游标
            buf.set_position(0);

            // 解析帧
            let frame = Frame::parse(&mut buf)?;

            // 从缓冲区中丢弃帧
            self.buffer.advance(len);

            // 返回帧的调用者
            Ok(Some(frame))
        }
        // 没有足够数据被缓存的情况
        Err(Incomplete) => Ok(None),
        // 一个错误被捕获
        Err(e) => Err(e.into()),
    }
}
```

完整的 [Frame::check](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/frame.rs#L63-L100) 函数代码可在这里找到. 我们不会完全的介绍它.
需要注意的是`Buf`使用了"字节迭代器"风格的API. 它们获取数据并推进游标. 比如, 为了解析一个帧,检查第一个字节来确定帧的类型. 这样的功能使用
[Buf::get_u8](https://docs.rs/bytes/0.6/bytes/buf/trait.Buf.html#method.get_u8) . 它会获取游标位置的字节,并将游标前进一位.

在[Buf](https://docs.rs/bytes/0.6/bytes/buf/trait.Buf.html) trait上还有更多有用的方法. 查看[API docs](https://docs.rs/bytes/0.6/bytes/buf/trait.Buf.html) 来了解更多细节.

## 缓冲写(Buffered writes)
帧相关的API另外一半是`write_frame(frame)`函数. 此函数将整个帧写入到socket中. 为了最小化`write`的系统调用, 写入将先被缓冲. 在写入到socket之前,
将维持一个写缓冲区并将帧编码到此缓冲区.

考虑到大数据量(bulk)的帧流. 要使用`Frame::Bulk(Bytes)`来写入. bulk帧有一个帧头, 它由`$`符后跟数据长度(以字节为单位)组成. 帧的大部分都是`Bytes`值的内容.
如果数据很大,则将其复制到中间缓冲区将会是非常昂贵的操作.

为了实现写缓冲, 我们将使用 [BufWriter struct](https://docs.rs/tokio/0.3/tokio/io/struct.BufWriter.html) . 这个结构体使用`T:AsyncWrite`初始化,
并自身实现了`AsyncWrite`. 当在`BufWriter`上调用`write`时,写操作不会直接传递给内部写程序,而是传递给缓冲区. 当缓冲区满时, 内容会刷新到内部写入器,并清除内部缓冲区.
在某些情况下,还有一些优化可以绕过缓冲区直接写到内部写入器.

在本指引的这部分,我们将不会去尝试实现一个完整的`write_frame()`功能. 完整的实现请查看[这里](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/connection.rs#L159-L184).

首先更新`connection`结构体:

```rust
use tokio::io::BufWriter;
use tokio::net::TcpStream;
use bytes::BytesMut;

pub struct connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}

impl connection {
    pub fn new(stream: TcpStream) -> connection {
        connection {
            stream: BufWriter::new(stream),
            buffer: BytesMut::with_capacity(4096),
        }
    }
}
```

然后, 实现`write_frame()`.

```rust
use tokio::io::{self, AsyncWriteExt};
use mini_redis::Frame;

async fn write_value(&mut self, frame: &Frame)
    -> io::Result<()>
{
    match frame {
        Frame::Simple(val) => {
            self.stream.write_u8(b'+').await?;
            self.stream.write_all(val.as_bytes()).await?;
            self.stream.write_all(b"\r\n").await?;
        }
        Frame::Error(val) => {
            self.stream.write_u8(b'-').await?;
            self.stream.write_all(val.as_bytes()).await?;
            self.stream.write_all(b"\r\n").await?;
        }
        Frame::Integer(val) => {
            self.stream.write_u8(b':').await?;
            self.write_decimal(*val).await?;
        }
        Frame::Null => {
            self.stream.write_all(b"$-1\r\n").await?;
        }
        Frame::Bulk(val) => {
            let len = val.len();

            self.stream.write_u8(b'$').await?;
            self.write_decimal(len as u64).await?;
            self.stream.write_all(val).await?;
            self.stream.write_all(b"\r\n").await?;
        }
        Frame::Array(_val) => unimplemented!(),
    }

    self.stream.flush().await;

    Ok(())
}
```

此处使用的功能由`AsyncWriteExt`提供. 它们也可以在`TcpStream`上使用,但不建议在没有中间缓冲区的情况下发出单字节写操作.

* [write_u8](https://docs.rs/tokio/0.3/tokio/io/trait.AsyncWriteExt.html#method.write_u8) 写入单个字节到writer上.
* [write_all](https://tokio.rs/tokio/tutorial/framing) 将整个切片写到writer上.
* [write_decimal](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/connection.rs#L225-L238) 由mini-redis来实现.

函数的末尾调用`self.stream.flush().await`. 是因为`BufWriter` 将写操作存储到中间缓冲区上, 因此写调用不能保证将数据写到socket中.
在返回前,我们希望装饰帧写入到socket中. 调用`flush()`将缓冲区中的所有数据写入到socket中.

另外一种二选一的方法是,不在`write_frame()`中调用`flush()`. 而是在`connection`上提供`flush()`函数. 这将允许调用者将队列中的多个小帧
写入到队列, 然后使用系统调用写入将它们全写入到socket中. 但这样做会使用`Coonection`API变得复杂. 简洁是Mini-Redis的目标之一, 因此我们决定在
`fn write_frame()` 中包含`flush().await`的调用.



&larr; [I/O](IO.md)

&rarr; [深入异步](AsyncInDepth.md)