use tokio::net::{TcpListener, TcpStream};
use mini_redis::{Connection, Frame};

#[tokio::main]
async fn main() {
    // 绑定监听器到一个地址
    let mut listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    // 循环接收 socket 链接
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // process(socket).await; 如果直接处理，那么每一次只能处理一个新链接
        // 将为每一个入站的新socket链接产生一个新task去处理它
        tokio::spawn(async move {
           process(socket).await;
        });
    }
}

async fn process(socket: TcpStream) {
    let mut connection = Connection::new(socket);
    // "链接" 可以让我们通过字节流 读/写 redis的 **帧**. "链接" 类型被 mini-redis 定义.
    if let Some(frame) = connection.read_frame().await.unwrap() {
        println!("GOT: {:?}", frame);

        // 返回一个错误
        let response = Frame::Error("unimplemented".to_string());
        connection.write_frame(&response).await.unwrap();
    }
}