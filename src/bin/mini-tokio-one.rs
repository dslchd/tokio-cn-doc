use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use futures::task;
use std::option::Option::Some;
use std::time::{Instant, Duration};

mod common;

use common::delay::Delay;

/// mini-tokio 第一版
fn main() {
    let mut mini_tokio = MiniTokio::new();

    mini_tokio.spawn(async {
        let when = Instant::now() + Duration::from_millis(10);
        let future = Delay { when };

        let out = future.await;

        println!("out result: {}", out);
    });

    mini_tokio.run(); // 如果没执行这一句，将不会有任何的结果
}

struct MiniTokio {
    tasks: VecDeque<Task>,
}

type Task = Pin<Box<dyn Future<Output=()> + Send>>;


impl MiniTokio {
    // 初始化一个MiniTokio 对象
    fn new() -> Self {
        MiniTokio { tasks: VecDeque::new() }
    }

    // 在mini-tokio实例上产生一个future
    fn spawn<F>(&mut self, future: F)
        where
            F: Future<Output=()> + Send + 'static,
    {
        self.tasks.push_back(Box::pin(future));
    }

    fn run(&mut self) {
        // 创建一个新waker
        let waker = task::noop_waker();
        let mut context = Context::from_waker(&waker);

        // 循环人队列中拿任务task 并匹配，
        while let Some(mut task) = self.tasks.pop_front() {
            if task.as_mut().poll(&mut context).is_pending() {
                self.tasks.push_back(task);
            }
        }
    }
}