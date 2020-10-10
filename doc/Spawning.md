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
use mini_redis::{Connection, Frame};

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
    let mut connection = Connection::new(socket);

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
