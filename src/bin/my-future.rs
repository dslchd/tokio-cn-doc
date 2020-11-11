mod common;
use std::time::{Instant, Duration};
use common::delay::Delay;

#[tokio::main]
async fn main() {
    // when = 现在的时间戳+ 10ms
    let when = Instant::now() + Duration::from_millis(10);
    // 初始化一个Delay
    let future = Delay{when};

    println!("Before future.await call");

    let out = future.await;

    println!("future.await result : {}", out)
}
