use crossbeam::channel;
use std::sync::{Arc, Mutex};
use std::pin::Pin;
use futures::{task, task::ArcWake, Future};
use std::task::{Context};
use std::time::{Instant, Duration};

mod common;
use common::delay::Delay;

fn main() {
    let mut mini_tokio = MiniTokio::new();
    mini_tokio.spawn(async {
        println!("这一句先打印出来!");
        let when = Instant::now() + Duration::from_millis(10);
        let future = Delay { when };

        let out = future.await;

        println!("out result: {}", out);
    });
    mini_tokio.run();
}


struct Task {
    future: Mutex<Pin<Box<dyn Future<Output=()> + Send>>>,
    executor: channel::Sender<Arc<Task>>,
}

impl Task {
    fn schedule(self: &Arc<Self>) {
        let _ = self.executor.send(self.clone());
    }

    fn poll(self: Arc<Self>) {
        // 从task实例上创建一个waker. 它使用 ArcWake
        let waker = task::waker(self.clone());
        let mut context = Context::from_waker(&waker);
        // 没有其它线程试图锁住 future，所以可以try_lock()
        let mut future = self.future.try_lock().unwrap();

        // 轮询future
        let _ = future.as_mut().poll(&mut context);
    }

    /// 使用指定的future产生一个新的任务
    ///
    /// 初始化一个新的task,它包含了future，完成后将task 推送到队列中, channel的另外一半receiver将接收到它们.
    fn spawn<F>(future: F, sender: &channel::Sender<Arc<Task>>)
        where F: Future<Output=()> + Send + 'static,
    {
        let task = Arc::new(Task {
            future: Mutex::new(Box::pin(future)),
            executor: sender.clone(),
        });

        let _ = sender.send(task);
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

    // 此run方法，将会一直执行
    fn run(&self) {
        while let Ok(task) = self.scheduled.recv() {
            task.poll();
        }
    }

    fn new() -> Self {
        let (sender, scheduled) = channel::bounded(100);
        MiniTokio { sender, scheduled }
    }

    /// MiniTokio 产生一个future
    fn spawn<F>(&mut self, future: F)
        where F: Future<Output=()> + Send + 'static
    {
        Task::spawn(future, &self.sender)
    }
}
