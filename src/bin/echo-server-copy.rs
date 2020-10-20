use tokio::io;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut listener = TcpListener::bind("127.0.0.1:6124").await.unwrap();
    loop {
        let (mut socket,_) = listener.accept().await?;
        tokio::spawn(async move{
            let (mut rd, mut wr) = socket.split();
            // 打印一下
            // 使用io::copy 直接将reader中的数据复制到writer中去
            if io::copy(&mut rd, &mut wr).await.is_err() {
                eprintln!("failed to copy");
            }
        });
    }
}