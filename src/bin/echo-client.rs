use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> io::Result<()> {
    // 链接到一个server
    let tcp_stream = TcpStream::connect("127.0.0.1:6124").await?;

    let (mut rd, mut wr)  = io::split(tcp_stream);

    let _write_task = tokio::spawn(async move {
        wr.write_all(b"hello\r\n").await?;
        wr.write_all(b"world\r\n").await?;

        Ok::<_, io::Error>(())
    });


    let mut buf = vec![0; 32];

    // 循环读取从服务端返回的数据
    loop {
        let n = rd.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        println!("GOT message from server {:?}", String::from_utf8(Vec::from(&buf[..n])));
        buf.clear();
    }

    Ok(())
}