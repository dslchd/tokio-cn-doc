use mini_redis::{client, Result};

#[tokio::main]
pub async fn main() -> Result<()> {
    // 打开一个链接到mini-redis地址的链接.
    let server_address:&str = "127.0.0.1:6379";
    let mut client = client::connect(server_address).await?;

    // 设置 hello 的值为 world
    client.set("hello", "world".into()).await?;

    // 获取 hello 的值
    let result = client.get("hello").await?;

    println!("got value from server; result={:?}", result);

    Ok(())
}