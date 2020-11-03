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
use std::cell::RefCell;

// 用于跟踪当前的mini-tokio实例,来使 spawn 函数能调度产生的实例.
thread_local! {
    static CURRENT: RefCell<Option<channel::Sender<Arc<Task>>>> = RefCell::new(None);
}

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


/// 一个基于channel的非常基础的futures 执行器(executor). 当任务(task)被唤醒时,它们通过在channel的发送方
/// 中来排队调度. 执行器在接收方(receiver)等待并执行接收到的任务.
///
/// 当一个任务被执行时，channel的发送方(sender)传递任务的Waker.
struct MiniTokio {
    // 接收调度的任务. 当一个任务被安排(或调度)时, 与之相关的future会准备好推进. 这通常发生在资源任务准备执行操作的时候
    // 比如说 一个socket接收到数据且一个 read 将调用成功时.
    scheduled: channel::Receiver<Arc<Task>>,
    sender: channel::Sender<Arc<Task>>,
}


impl MiniTokio {
    // 初始化一个新的mini-tokio实例
    fn new() -> MiniTokio {
        let (sender, scheduled) = channel::bounded(1000);
        MiniTokio{scheduled, sender}
    }

    // 在mini-tokio实例上产生一个future
    // 给future 包装task 并将其推送到 scheduled 队列中去,当run方法被调用时future将会执行
    fn spawn<F>(&mut self, future: F)
    where F:Future<Output = ()> + Send + 'static,
    {
        Task::spawn(future, &self.sender)
    }

    /// 运行执行器
    ///
    /// 这将启动执行器循环，并无限的运行，没有实现关机的机制
    ///
    /// 任务从 scheduled 通道的 receiver方出来. 在channel上接收一个任务表明任务已经准备好被执行了.
    /// 这发生在任务首次被创建和任务被唤醒时.
    fn run(&self) {
        println!("execute MiniTokio run method!");

        // 设置 CURRENT 线程局部变量来指向当前执行器
        // tokio 使用一个thread local变量来实现 `tokio::spawn`.
        CURRENT.with(|cell|{
            *cell.borrow_mut() = Some(self.sender.clone());
        });

        while let Ok(task) = self.scheduled.recv() {
            task.poll();
        }
    }
}

