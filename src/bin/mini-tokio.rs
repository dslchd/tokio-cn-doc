//! 演示了如何实现一个非常基础的异步Rust执行器与计时器.
use std::pin::Pin;
use std::time::{Instant, Duration};
use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::option::Option::Some;
use std::result::Result::Ok;
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use std::thread;
// 使用一个channel队列来调度tasks, 之所以不用std中的channel是因为std中的channel不是 Sync的，无法在线程中共享
use crossbeam::channel;
// 允许我们不使用 “不安全” 的代码来实现一个 `sta::task::waker` 功能的 工具
use futures::task::{ArcWake, self};

// 用于跟踪当前的mini-tokio实例,来使 spawn 函数能调度产生的实例.
thread_local! {
    static CURRENT: RefCell<Option<channel::Sender<Arc<Task>>>> = RefCell::new(None);
}

/// main 函数入口，创建一个mini-tokio实例，并产生一些任务(Task). 我们的mini-tokio实现仅支持产生task和设置delays
///
/// 此MiniTokio，源于官方指南中的MiniTokio 原理代码，原代码链接 [mini-tokio](https://github.com/tokio-rs/website/blob/master/tutorial-code/mini-tokio/src/main.rs)
fn main() {
    // 创建一个新MiniTokio实例
    let mut mini_tokio = MiniTokio::new();

    // 产生一个root 根据任务，所有其它的tasks都来自于这个上下文.  直到mini_tokio.run()调用之前，不会执行任何工作.
    mini_tokio.spawn(async {
        // 产生一个任务
        spawn(async {
            // 等待一小段时间以便 world在 hello后面打印
            delay(Duration::from_millis(1000)).await;
            println!("world")
        });

        // 产生第二个任务
        spawn(async {
           println!("hello");
        });

        //我们没有实现执行器的shutdown功能，因此这个强制关闭
        delay(Duration::from_secs(2)).await; // 2秒后关闭吧
        std::process::exit(0);
    });


    // 启动mini-tokio 执行器循环，调度任务并接收执行结果
    mini_tokio.run();

    mini_tokio.run();
}

/// 此spawn函数功能与tokio::spawn()一样. 当进行到mini-tokio执行器(executor)中时,
/// 'CURRENT' 本地线程(thread-local) 被设置指向执行器 channel的 Send 方. 然后，产生task需要为创建的"Task"套上
/// 一个"future" 并将其推到调度队列里面.
pub fn spawn<F>(future: F)
where F: Future<Output =()> + Send + 'static,
{
    CURRENT.with(|cell| {
        let borrow = cell.borrow();
        let sender = borrow.as_ref().unwrap();
        Task::spawn(future, sender);
    });
}

/// task 包含一个future和一旦future被唤醒后所必须要的数据
struct Task {
    // future使用 Mutex 来包装可以使用Task具有Sync 特性.
    // 仅有一个线程可以使用future.
    // 真实tokio运行时，没有使用Mutex这种排它锁，而是使用了unsafe代码. box也被避免使用了.
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    // 当task被通知时，它被发送到队列中去. 执行器通过取出通知任务来执行它们
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
        let _ = sender.send(task);
    }

    // 执行调度任务. 它创建了必须的 `task::Context`上下文，此Context,包含了一个waker与task.
    // 使用waker对future进行poll.
    fn poll(self: Arc<Self>) {
        // 从task实例上创建一个waker, 它使用了 ArcWake
        let waker = task::waker(self.clone());
        // 使用waker来初始化task的上下文
        let mut cx = Context::from_waker(&waker);

        // 这里绝不会阻塞，因为只有一你上线程能锁住future
        let mut future = self.future.try_lock().unwrap();

        // 轮询future
        let _ = future.as_mut().poll(&mut cx);
    }
}

// 在标准库中使用了一个低级别的API来定义waker,此API是unsafe的，为了不写unsafe代码，这里我们使用futures包提供的
// ArcWake 来定义一个waker，它可以被 Task结构体来调度.
impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // 调度Task来执行.执行器从channel中接收Task并poll Task.
        let _ = arc_self.executor.send(arc_self.clone());
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


// 异步等待，其作用相当于 thread::sleep. 尝试在当前函数上暂停指定的时间
//
// mini-tokio 通过一个计时器线程(timer thread) 来实现Delay, sleep 指定的duration后，一旦delay完成，就会
// 通知调用者. 每次调用delay都会产生一个线程. 显然这不是一个好的实施策略，没有人会将这种方法用在生产上(这里只是为了示例tokio原理)
// 真实的tokio没有使用这种策略.
async fn delay(dur: Duration) {

    // delay 在这里是一种片面的future描述. 有时候，它被当作一种 "resource"(资源). 其它的资源包括,socket与channels.
    // resource 可能不是按 async/await 来实现的，因为它们必须与一些操作系统细节合并. 因为这一原因，我们必须手动来实现 future
    //
    // 不过，最好将API公共为一个 async fn . 一个有用的方式是，手动定义私有future,然后从公共(pub)的`async fn`中使用它的API.

    struct Delay {
        // delay什么时候完成
        when: Instant,
        // 一旦delay完成后就会通知waker. waker必须能被timer线程与future访问，所以它使用Arc<Mutex<>>来包装.
        waker: Option<Arc<Mutex<Waker>>>,
    }

    // 为Delay 实现 Future trait
    impl Future for Delay {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            // 首先，如果第一次future就被调用，则产生一个timer线程. 如果timer线程已经在运行了，要确保存储的waker
            // 能匹配上当前task的waker.
            // 看是否能匹配上当前task的waker
            if let Some(waker) = &self.waker {
                let mut waker = waker.lock().unwrap();

                if !waker.will_wake(cx.waker()) {
                    *waker = cx.waker().clone();
                }
            }else {
                // 如果不能匹配上,创建一个waker
                let when = self.when;
                let waker = Arc::new(Mutex::new(cx.waker().clone()));
                // 赋值给当前task的waker
                self.waker = Some(waker.clone());

                // 每一次poll 被调用，产生一个timer线程
                thread::spawn(move ||{
                   let now = Instant::now();
                    // 如果还没到时间
                    if now < when {
                        // 睡眠一下剩余的时间
                        thread::sleep(when - now);
                    }
                    // 如果duration时间过去后，再通过激活waker来通知调用者
                    let waker = waker.lock().unwrap();
                    waker.wake_by_ref();
                });
            }

            // 一旦waker被存储且timer线程已经开始时，此时就要来检查delay是否已经完成了. 这通过当前时间来检查.
            // 如果duration时间已过，Future就完成了，此时返回Poll::Ready()
            if Instant::now() >= self.when {
                // 说明delay duration时间已经过去
                Poll::Ready(()) // 返回Poll::Ready()
            }else {
                // 时间没过返回Poll::Pending
                Poll::Pending
            }
        }
    }

    // 回到delay function中, 初始化一个Delay实全
    let future = Delay {
        when: Instant::now() + dur,
        waker: None, // 初始时并没有waker，它由poll去创建
    };

    // 等待duration完成
    future.await;
}
