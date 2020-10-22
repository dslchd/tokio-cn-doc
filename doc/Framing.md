## 帧(Framing)
现在,我们将应用刚刚学到的的I/O知识,并以此来实现Mini-Redis的帧层. 形成帧是获取字节流水并将其转换为帧流的过程. 帧是两个对等体之间传输数据的单位.
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

注意帧仅由没有任何语义的数据组成. 命令的解析和实现发生成更新的层级.

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

为了去实现Mini-Redis的帧, 我们将实现一个`Connection`结构体, 该结构体包装了一个`TcpStream`并读取/写入`mini_redis::Frame`的值.

```rust
use tokio::net::TcpStream;
use mini_redis::{Frame, Result};

struct Connection {
    stream: TcpStream,
    // ... 其它属性字段
}

impl Connection {
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

你能在[这里](https://redis.io/topics/protocol) 找到完整的Redis协议细节. 完整的 `Connection` 代码在 [这里](https://github.com/tokio-rs/mini-redis/blob/tutorial/src/connection.rs) .

## 缓冲区读取(Buffered reads)
`read_frame` 方法在返回之前会等待接收整个帧. 单次调用`TcpStream::read()`方法可能会返回任意数量的数据. 它可能包含一个完整的帧,一部分帧,或者多个帧.
如果接收到部分帧,则会被缓存,并从socket套接字中读取更多的数据. 如果接收到多个帧, 则返回第一个帧,并缓冲其它的数据,直到下一次调用`read_frame`为止.

为了实现这一点, `Connection`需要一个读取缓冲区字段. 数据从socket中读取到读缓冲区(read buffer)中. 当一个帧被解析时, 相应的数据就会从缓冲区中移除.

我们将使用 [BytesMut](https://docs.rs/bytes/0.5/bytes/struct.BytesMut.html) 作为缓冲区(buffer)的类型. 这是一个 [Bytes](https://docs.rs/bytes/) 的可变版本.

```rust
use bytes::BytesMut;
use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
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
        // 如果成功了, 一定数据的字节被返回. '0' 表明到了流水的末尾
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

pub struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
    cursor: usize,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            // 分配4kb的缓冲区容量
            buffer: vec![0; 4096],
            cursor: 0,
        }
    }
}
```

`Connection`上的`read_frame()` 函数.

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
需要数据写入的类型实现. 当传递一个 `T:BufMut` 到 `read_buf()`时, 缓冲区的内部游标由`read_buf()`自动更新. 