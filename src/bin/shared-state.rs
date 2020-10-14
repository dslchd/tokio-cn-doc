use tokio::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use bytes::Bytes;
use mini_redis::{Connection, Frame, Command};
use std::option::Option::Some;
use mini_redis::cmd::Command::{Set, Get};

type Db = Arc<Mutex<HashMap<String, Bytes>>>;

#[tokio::main]
async fn main() -> std::io::Result<()>{
    // 声明一个listener 并绑定到指定地址的一个端口上
    let mut listener = TcpListener::bind("127.0.0.1:6379").await?;
    println!("Listening localhost and port 6379");

    let db = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (socket, _) = listener.accept().await?;
        // clone
        let db = db.clone();
        println!("Accepted");
        // 处理socket
        process(socket, db).await;
    }

}

/// 处理函数
async fn process(socket: TcpStream, db: Db) {
    let mut connection = Connection::new(socket);

    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response  = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                let mut db = db.lock().unwrap();
                db.insert(cmd.key().to_string(), cmd.value().clone());
                // 返回 frame
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                let db = db.lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    Frame::Bulk(value.clone())
                }else {
                    Frame::Null
                }
            }
            // 其它cmd 情况
            cmd=> panic!("unimplemented Command :{:?}", cmd),
        };
        connection.write_frame(&response).await.unwrap();
    }
}