use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        let _ = tx1.send("one");
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    // 谁先完成返回谁，其它的丢弃
    tokio::select! {
        va1 = rx1 => {
            println!("rx1 completed first with {:?}", va1);
        }

        va1 = rx2 => {
            println!("rx2 completed first with {:?}", va1);
        }
    }

}