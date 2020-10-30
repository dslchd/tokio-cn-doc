use std::time::Instant;
use std::future::Future;
use std::pin::Pin;
use std::task::{Poll, Context};
use std::thread;

pub struct Delay {
    pub when: Instant,
}

impl Future for Delay {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

        if Instant::now() >= self.when {
            println!("hello world");
            Poll::Ready("done")
        }else {
            // 在当前任务上获取一个waker句柄
            let waker = cx.waker().clone();
            let when = self.when;

            // 产生一个定时器线程
            thread::spawn(move || {
               let now = Instant::now();

                if now < when {
                    // 说明还没到时间
                    thread::sleep(when - now);
                }

                waker.wake();
            });
            Poll::Pending
        }
    }
}