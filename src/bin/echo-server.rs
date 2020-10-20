use tokio::io;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> io::Result<()> {
    // 绑定一个地址与端口
    let mut listener = TcpListener::bind("127.0.0.1:6124").await.unwrap();
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            // 缓存buffer 手动复制内容到writer中
            let mut buf:Vec<u8> =vec![0;32];
            loop {
                match socket.read(&mut buf).await {
                    // 如果是 Ok(0) 表示远程已经关闭链接, 那就直接return
                    Ok(0) => return,
                    Ok(n) => {
                        // 复制数据返回到socket中去, 模式匹配写回并判断是否出错了
                        println!("Receive data: {:?} from client", String::from_utf8(Vec::from(&buf[..n])));
                        if socket.write_all(&buf[..n]).await.is_err() {
                            // 未期待的错误,不处理直接返回
                            return;
                        }
                    }
                    Err(_) => return
                }
            }
        });
    }
}

