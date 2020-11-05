use tokio::sync::oneshot;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    let mut out = String::new();

    tokio::spawn(async move {
        let _ = tx1.send("hello");
    });
    tokio::spawn(async move {
        let _ = tx2.send("world");
    });

    tokio::select! {
        Ok(str) = rx1 => {
            out.push_str(format!("rx1 completed: {}", str).as_str());
        }

        Ok(str) = rx2 => {
            out.push_str(format!("rx2 completed: {}", str).as_str());
        }
    }
    println!("{}", out);
    Ok(())
}