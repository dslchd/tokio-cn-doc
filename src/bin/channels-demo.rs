use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};
use std::option::Option::Some;
use mini_redis::client;

/// 由请求者提供并通过管理任务来发送,再将命令的响应返回给请求者.
type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;

#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        val: Vec<u8>,
        resp: Responder<()>,
    },
}


/// 示例配合 store-value 服务端使用， 或者按前面教程里 使用 mini-redis-server 服务端
#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = mpsc::channel(32);

    // 产生一个管理任务
    let manager = tokio::spawn(async move {
        //建立一个与服务器的链接
        let mut client = client::connect("127.0.0.1:6379").await.unwrap();

        // 开始接收redis server 那边的消息
        while let Some(message) = rx.recv().await {
            use Command::*;
            // 匹配redis返回的 命令类型
            match message {
                Get { key, resp } => {
                    let res = client.get(&key).await;
                    let _ = resp.send(res);
                }
                Set { key, val, resp } => {
                    let res = client.set(&key, val.into()).await;
                    let _ = resp.send(res);
                }
            }
        }
    });

    // clone 发送者完成多个任务的发送
    let mut tx2 = tx.clone();

    let t1 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Get {
            key: "hello".to_string(),
            resp: resp_tx,
        };
        // 发送 GET 请求
        if tx.send(cmd).await.is_err() {
            eprintln!("t1 connection task shutdown");
            return;
        }
        // 等待响应
        let res = resp_rx.await;
        println!("T1 GOT = {:?}", res);
    });

    let t2 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Set {
            key: "hello".to_string(),
            val: "world".into(),
            resp: resp_tx,
        };
        if tx2.send(cmd).await.is_err() {
            eprintln!("t2 connection task shutdown");
            return;
        }
        // 等待响应
        let res = resp_rx.await;
        println!("T2 GOT = {:?}", res);
    });

    // while let Some(message) = rx.recv().await {
    //     println!("Got = {}", message);
    // }

    t1.await.unwrap();
    t2.await.unwrap();
    manager.await.unwrap();
}