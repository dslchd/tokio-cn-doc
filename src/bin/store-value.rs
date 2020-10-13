use tokio::net::{TcpStream, TcpListener};
use mini_redis::{Connection, Frame};

async fn process(socket: TcpStream) {
    use mini_redis::Command::{self, Get, Set};
    use std::collections::HashMap;

    // 声明一个用来存储数据的hashmap
    let mut db = HashMap::new();

    // 此connection 由mini_redis包提供,　可以处理socket中的　帧
    let mut connection = Connection::new(socket);

    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                if let Some(value) = db.get(cmd.key()) {
                    // Frame::Bulk() 里面要是一个Bytes 使用 into() 转换为 Bytes
                    Frame::Bulk(value.clone().into())
                }else {
                    Frame::Null
                }
            }
            // 其它的命令没有实现
            cmd=> panic!("unimplemented {:?}", cmd),
        };
        // 写入响应到客户端
        connection.write_frame(&response).await.unwrap();
    }
}

#[tokio::main]
async fn main() {
    // 绑定监听器到一个地址
    let mut listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    println!("Mini Redis Server started, listen port: {}", 6379);
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            process(socket).await;
        });
    }
}