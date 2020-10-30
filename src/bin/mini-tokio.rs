//! 演示了如何实现一个非常基础的异步Rust执行器与计时器.
mod common;
use std::collections::VecDeque;
use futures::{task, Future};
use std::pin::Pin;
use std::time::{Instant, Duration};
use common::delay::Delay;
use std::task::Context;
use std::option::Option::Some;
// 使用一个channel队列来调度tasks, 之所以不用std中的channel是因为std中的channel不是 Sync的，无法在线程中共享
use crossbeam::channel;
use std::sync::{Arc, Mutex};
// 允许我们不使用 “不安全” 的代码来实现一个 `sta::task::waker` 功能的 工具
use futures::task::ArcWake;
use std::result::Result::Ok;

fn main() {
    let mut mini_tokio = MiniTokio::new();

    mini_tokio.spawn(async {
        let when = Instant::now() + Duration::from_millis(100);
        let futures = Delay {when};

        let out = futures.await;
        println!("out :{}", out);
    });

    mini_tokio.run();
}

struct Task {
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    executor: channel::Sender<Arc<Task>>,
}

impl Task {
    // 使用指定的future产生一个新的future
    // 初始化一个新的包含了指定future的task，并将其它推送给 sender. channel另外一半的receiver将接收到它并执行.
    fn spawn<F>(future: F, sender: &channel::Sender<Arc<Task>>)
        where F: Future<Output = ()> + Send + 'static,
    {
        let task = Arc::new(Task {
            future: Mutex::new(Box::pin(future)),
            executor: sender.clone(),
        });
        sender.send(task);
    }

    fn poll(self: Arc<Self>) {
        // 从task实例上创建一个waker, 它使用了 ArcWake
        let waker = task::waker(self.clone());
        let mut cx = Context::from_waker(&waker);

        // 没有其它线程试图锁住future
        let mut future = self.future.try_lock().unwrap();

        // 轮询future
        let _ = future.as_mut().poll(&mut cx);
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
       arc_self.schedule();
    }
}


struct MiniTokio {
    scheduled: channel::Receiver<Arc<Task>>,
    sender: channel::Sender<Arc<Task>>,
}


impl MiniTokio {
    // 初始化一个新的mini-tokio实例
    fn new() -> MiniTokio {
        let (sender, receiver) = channel::bounded(1000);
        MiniTokio{scheduled: receiver, sender}
    }

    // 在mini-tokio实例上产生一个future
    // 给future 包装task 并将其推送到 scheduled 队列中去
    fn spawn<F>(&mut self, future: F)
    where F:Future<Output = ()> + Send + 'static,
    {
        Task::spawn(future, &self.sender)
    }

    fn run(&self) {
        println!("execute MiniTokio run method!");

        while let Ok(task) = self.scheduled.recv() {
            task.poll();
        }
    }
}
