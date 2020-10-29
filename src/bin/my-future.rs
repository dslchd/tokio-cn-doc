use std::time::Instant;
use std::future::Future;
use futures::task::Context;
use tokio::macros::support::{Pin, Poll};
use tokio::time::Duration;

struct Delay {
    when: Instant,
}

impl Future for Delay {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

        if Instant::now() >= self.when {
            println!("hello world");
            Poll::Ready("done")
        }else {
            // wake_by_ref() : 唤醒与waker相关的任务，而不去消费waker
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[tokio::main]
async fn main() {
    // when = 现在的时间戳+ 10ms
    let when = Instant::now() + Duration::from_millis(10);
    // 初始化一个Delay
    let future = Delay{when};

    let out = future.await;

    assert_eq!(out, "done");
}
