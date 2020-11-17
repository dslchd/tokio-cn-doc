use tokio::net::TcpStream;
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (sender, receiver) = oneshot::channel();

    // 生产一个任务来发送消息到 oneshot中去
    tokio::spawn(async move {
        sender.send("one").unwrap();
    });

    tokio::select! {
        // 这里不会被匹配上
        socket = TcpStream::connect("localhost:3465") => {
            println!("Socket connected {:?}", socket);
        }

        msg = receiver => {
            println!("receiver message first: {:?}", msg);
        }
    }
}