## Select
到目前为止，当我们想向系统添加并发时，我们会产生一个新的任务(task). 现在我们将介绍使用Tokio来并发执行异步代码的其它方法.

## `tokio::select!`
`tokio::select!` 宏允许等待多个异步计算且当单个计算完成时返回(译者注: 多个并发或并行异步计算任务，返回最先完成的那个).

比如说:

```rust
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        let _ = tx1.send("one");
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
}
```

使用了两个 `oneshot` 通道. 其中任一通道都能先完成. `select!` 语句在两个channels上等待,并将`va1`绑定到任务返回的值上. 当其中任一 `tx1` 或者
`tx2` 完成时，与之相关的块就会执行.

另外没有被完成的分支将会被丢弃(dropped). 在上面的示例中，计算正在每个channel的 `oneshot::Receiver` 上等待. 没有完成的`oneshot::Receiver`
channel将会被丢弃.

### 取消(Cancellation)
对于异步Rust来说，取消操作是通过删除一个future来完成的. 回顾一下 [深入异步](AsyncInDepth.md) 章节中，使用future来实现Rust的异步操作且
future是惰性的. 仅仅当future被轮询时操作才会处理. 如果future被删除(丢弃)，操作就不会继续，因为与之所有相关联的状态都被丢弃了.

也说是说，有时候异步操作将产生后台任务或者启动在后台运行的其它操作. 比方说，在上面的示例中，产生一个任务将消息发送回去. 一般来说这个任务会执行
一些计算来生成值.

Futures或者其它类型能通过实现 `Drop` 去清理后台资源. Tokio的`oneshot::Receiver`通过向`Sender`方发送一个关闭的通知来实现`Drop`功能.
Sender方能接收到这个通知并通过丢弃正在进行的操作来中止它.

```rust
use tokio::sync::oneshot;

async fn some_operation() -> String {
    // 这里计算值
}

#[tokio::main]
async fn main() {
    let (mut tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        // select 操作和 oneshot 的 `close()` 通知.
        tokio::select! {
            val = some_operation() => {
                let _ = tx1.send(val);
            }
            _ = tx1.closed() => {
                // `some_operation()` 被调用, 
                // 任务完成且 `tx1` 被丢弃
            }
        }
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
}
```

### `Future`的实现(The `Future` implementation)
为了帮助更好的理解`select!`是如何工作的，让我们看看假想的Future实现像什么样子. 这是一个简单的版本. 在具体的实践中，`select!`还包括其它的功能，
比如随机选择要首先轮询的分支.

```rust
use tokio::sync::oneshot;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct MySelect {
    rx1: oneshot::Receiver<&'static str>,
    rx2: oneshot::Receiver<&'static str>,
}

impl Future for MySelect {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if let Poll::Ready(val) = Pin::new(&mut self.rx1).poll(cx) {
            println!("rx1 completed first with {:?}", val);
            return Poll::Ready(());
        }

        if let Poll::Ready(val) = Pin::new(&mut self.rx2).poll(cx) {
            println!("rx2 completed first with {:?}", val);
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    // use tx1 and tx2

    MySelect {
        rx1,
        rx2,
    }.await;
}
```
`MySelect` future 包含每个分支的future. 当`MySelect`被轮询时，第一个分支被轮询. 如果它是ready状态，就使用它的值且`MySelect`完成.
然后`.await`接收到来着future的输出，future被删除. 结果就是两个分支的future都被删除. 因为一个分支未完成，因此操作被有效取消.

记住来自上一章节的话:

```markdown
当一个future返回Poll::Pending时，它**必须**确保在future的某个时候向waker发送信号. 忘记这样做会导致任务无限被挂起.
```

`MySelect`的实现中没有显示的使用`Context`的参数. 取代的是，通过将`cx`传递给内部future来满足waker的要求. 由于内部future也必须满足waker的
要求，因此在收到来自内部future的`Poll::Pending`时仅返回`Poll::Pending`. 所以`MySelect`也满足waker的要求.

## 语法(Syntax)
`select!`宏能处理超过2个以上的分支. 当前最大限制64个分支. 每个分支的结构像下面这样:

```text
<pattern> = <async expression> => <handler>,
```

当`select!`宏展开时，所有的`<async expression>`都会被汇总并同时执行. 当其中一个表达式完成时，结果就会被匹配到`<pattern>`. 如果结果与
pattern匹配时，那么将删除所有剩余的异步表达式并执行`<handler>`. `<handler>`表达式可以访问被`<pattern>`建立的任何绑定值.

基本上`<pattern>`就是变量名，异步表达式的结果可以绑定到这个变量名上且`<handler>`可以访问这个变量. 这就是为什么最开始的示例中，`va1`能被
`<pattern>`使用且`<handler>`能访问`va1`.

如果`<pattern>`与异步计算的结果不匹配，则其余的异步表达式将继续并发执行直到下一个完成为止. 这时，将相同的逻辑用于该结果.

因为`select!`可以采用任意的异步表达式，所以可以在定义复杂的计算时来选择它.

在这里，我们选择`oneshot` channel和TCP链接的输出.

```rust
use tokio::net::TcpStream;
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx, rx) = oneshot::channel();

    // 产生一个任务来发送消息到oneshot 中
    tokio::spawn(async move {
        tx.send("done").unwrap();
    });

    tokio::select! {
        socket = TcpStream::connect("localhost:3465") => {
            println!("Socket connected {:?}", socket);
        }
        msg = rx => {
            println!("received message first {:?}", msg);
        }
    }
}
```

在这里，我们选择一个`oneshot`并接收来自`TcpListener`的socket套接字.

```rust
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        tx.send(()).unwrap();
    });

    let mut listener = TcpListener::bind("localhost:3465").await?;

    tokio::select! {
        _ = async {
            loop {
                let (socket, _) = listener.accept().await?;
                tokio::spawn(async move { process(socket) });
            }

            // 帮助Rust的类型推导
            Ok::<_, io::Error>(())
        } => {}
        _ = rx => {
            println!("terminating accept loop");
        }
    }

    Ok(())
}
```

accept循环一直运行，直到遇到错误或`rx`接到到值为止. `_`表示我们对异步计算返回的值不感兴趣.

## 返回值(Return value)
`tokio::select!`宏返回`<handler>`表达式的结果.

```rust
async fn computation1() -> String {
    // 计算1
}

async fn computation2() -> String {
    // 计算2
}

#[tokio::main]
async fn main() {
    let out = tokio::select! {
        res1 = computation1() => res1,
        res2 = computation2() => res2,
    };

    println!("Got = {}", out);
}
```

因为这一点，它需要`<handler>`表达式每个分支返回的值相同. 如果`select!`表达式的输出不是必须的，推荐将表达式的返回值类型为 `()`.

## 错误(Errors)
使用`?`号操作符从表达式传播错误. 它如何工作是取决于是否`?`号从异步表达式或处理程序中使用. 使用`?`在异步表达式中能将错误传播到异步表达式之外.
这就使异步表达式的输出成一个`Result`了. 从一个处理程序使用`?`号能立即传播错误到`select!`表达式之外. 让我们再次来看看accept 循环:

```rust
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    // [设置 `rx` oneshot channel]
    let listener = TcpListener::bind("localhost:3465").await?;

    tokio::select! {
        res = async {
            loop {
                let (socket, _) = listener.accept().await?;
                tokio::spawn(async move { process(socket) });
            }

            // 帮助Rust类型推导
            Ok::<_, io::Error>(())
        } => {
            res?;
        }
        _ = rx => {
            println!("terminating accept loop");
        }
    }

    Ok(())
}
```

注意`listener.accept().await?`. `?`号操作符传播错误到表达式之外且和`res`绑定. 如果是一个错误, `res`将被设置为`Err(_)`. 当然在handler内部
`?`可以再次使用. `res?` 声明将传播一个错误到`main`函数之外.

## 模式匹配(Pattern matching)
回顾一下`select!`宏的分支语法定义:

```text
    <pattern> = <async expression> = <handler>,
```

到目前为止，我们仅仅对`<pattern>`使用了变量绑定. 然而，这里能使用任何Rust模式. 比如说，假设我们从多个 MPSC 通道接收，我们可能会执行以下操作:

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (mut tx1, mut rx1) = mpsc::channel(128);
    let (mut tx2, mut rx2) = mpsc::channel(128);

    tokio::spawn(async move {
        // Do something w/ `tx1` and `tx2`
    });

    tokio::select! {
        Some(v) = rx1.recv() => {
            println!("Got {:?} from rx1", v);
        }
        Some(v) = rx2.recv() => {
            println!("Got {:?} from rx2", v);
        }
        else => {
            println!("Both channels closed");
        }
    }
}
```

在这个例子中，`select!`表达式等待从`rx1`和`rx2`接收值. 如果一个channel关闭了，`recv()`返回了`None`. 这与模式不匹配且分支会被禁用.
`select!`表达将继续在其它分支上等待.

注意`select!`表达式包含了一个`else`分支. `select!`表达式必须返回一个值. 在使用模式匹配时，可能所有的分支都不能匹配上关联的模式. 如果这种
情况发生了，那么`else`分支将会被返回.

## 借用(Borrowing)
当产生一个任务时，生成的异步表达式必须要有其所有的数据. `select!`宏没有这样的限制. 每一个分支的数据都能借用数据并同时进行操作. 根据Rust的
借用规则来看,多个异步表达式可以，**不可变**的借用单个数据，或者单个异步表达式可以**可变**的借用数据.

让我们来看一些例子. 在这里，我们同时将相同的数据发送到两个不同的TCP目标上.

```rust
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use std::io;
use std::net::SocketAddr;

async fn race(
    data: &[u8],
    addr1: SocketAddr,
    addr2: SocketAddr
) -> io::Result<()> {
    tokio::select! {
        Ok(_) = async {
            let mut socket = TcpStream::connect(addr1).await?;
            socket.write_all(data).await?;
            Ok::<_, io::Error>(())
        } => {}
        Ok(_) = async {
            let mut socket = TcpStream::connect(addr2).await?;
            socket.write_all(data).await?;
            Ok::<_, io::Error>(())
        } => {}
        else => {}
    };

    Ok(())
}
```

这两个异步表达式中都是**不可变**的借用了`data`变量. 当其中一个操作成功完成后，另外一个将被丢弃. 因为我们在`Ok()`上进行了模式匹配，如果一个表达式
失败，另外一个将继续执行.

当涉及到每个分支的`<handler>`时，`select!`保证只有一个`<handler>`运行. 根据这一点，每一个`<handler>`可以**可变**的借用同一个数据.

例如，修改下两个handlers:

```rust
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    let mut out = String::new();

    tokio::spawn(async move {
        // 在 tx1和tx2上发送值
    });

    tokio::select! {
        _ = rx1 => {
            out.push_str("rx1 completed");
        }
        _ = rx2 => {
            out.push_str("rx2 completed");
        }
    }

    println!("{}", out);
}
```

## 循环(Loops)
`select!`宏经常在循环中使用. 本节将介绍一些示例，来展示在循环中使用`select!`的常用方法. 我们首先使用 multiple channels:

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx1, mut rx1) = mpsc::channel(128);
    let (tx2, mut rx2) = mpsc::channel(128);
    let (tx3, mut rx3) = mpsc::channel(128);

    loop {
        let msg = tokio::select! {
            Some(msg) = rx1.recv() => msg,
            Some(msg) = rx2.recv() => msg,
            Some(msg) = rx3.recv() => msg,
            else => { break }
        };

        println!("Got {}", msg);
    }

    println!("All channels have been closed.");
}
```

上面的示例选择了3个channel的接收器(receiver). 在任何通道上接收消息时，它将被写入到STDOUT. 当一个channel关闭时，`recv()`会返回`None`.
通过使用模式匹配，`select!`宏会继续在其它channel上等待. 当所有的通道都关闭了，`else`分支会被匹配且循环被终止.

`select!`宏会随机的选择分支来首先枪柄就绪情况. 当多个通道都有待定的值时，将从其中随机选择一个来接收. 这是为了处理接收循环处理消息的速度慢于将消息
推送到通道中的情况，这意味着通道填充数据. 如果`select!`没有随机的选择首先要检查的分支，那么在每次循环迭代中，将首先检查`rx1`. 如果`rx1`
始终都有新消息，则永远不会再检查其余的通道了.

```markdown
如果当`select!`被评估时，多个通道都有待处理的消息，只会弹出(pop)一个通道的值. 所有其它的通道保持不变，它们的消息会保留在这些通道中，直到下一次循环迭代为止. 不会有消息丢失.
```

### 恢复异步操作(Resuming an async operation)

现在，我们将展示如何在多个`select!`调用之间运行异步操作! 在这个示例当中，我们使用一个类型为`i32`的 MPSC channel，并且它是异步的. 我们要运行异步函数，直到它完成或在接收到偶数整数为止.

```rus
async fn action() {
    // 一些异步逻辑
}

#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = tokio::sync::mpsc::channel(128);    
    
    let operation = action();
    tokio::pin!(operation);
    
    loop {
        tokio::select! {
            _ = &mut operation => break,
            Some(v) = rx.recv() => {
                if v % 2 == 0 {
                    break;
                }
            }
        }
    }
}
```

注意如何，而不是在`select!`宏中调用`action()`， 它在循环外被调用. `action()`的返回分配给`operation`，而不需要调用`.await`.然后我们在`operation`上调用

`tokio::pin!`.

在`select!`循里面，不是传递`operation`而是传递`&mut operation`. `operation`变量正在跟踪异步操作. 循环中的每一次迭代都使用相同的操作，而不是发出对`action()`的一次新的调用.

其它的`select!`分支从通道中接收消息. 如果消息是偶数，则循环完成. 否则再次开始 `select!`.

这里我们第一次使用了`tokio::pin!`.  我们暂时不去讨论pin的细节. 需要注意的是，为了`.await`一个引用，必须固定引用的值或者实现`Unpin`.

如果我们移除`tokio::pin!`这一行并再去尝试编译，我们会得到下面的错误:

```text
error[E0599]: no method named `poll` found for struct
     `std::pin::Pin<&mut &mut impl std::future::Future>`
     in the current scope
  --> src/main.rs:16:9
   |
16 | /         tokio::select! {
17 | |             _ = &mut operation => break,
18 | |             Some(v) = rx.recv() => {
19 | |                 if v % 2 == 0 {
...  |
22 | |             }
23 | |         }
   | |_________^ method not found in
   |             `std::pin::Pin<&mut &mut impl std::future::Future>`
   |
   = note: the method `poll` exists but the following trait bounds
            were not satisfied:
           `impl std::future::Future: std::marker::Unpin`
           which is required by
           `&mut impl std::future::Future: std::future::Future`
```

这个错误不是很清晰，我们也没有讨论太多的关于`Future`的信息. 现在将`Future`看作必须通过一什值实现才能调用`.await`的trait. 如果在尝试对引用调用`.await`时遇到了有关没有实现`Future`的错误时，则可能需要固定住`Future`.

有关标准库中`Pin`更多的细节可以查看[Pin](https://doc.rust-lang.org/std/pin/index.html).

### 修改一个分支(Modifying a branch)

让我们看看一个稍微复杂的循环. 我们有:

1. `i32`值的通道.
2. 对`i321值执行的异步操作.

我们想实现的逻辑是:

1. 在channel上等待一个偶数.
2. 使用偶数作为输入启动异步操作.
3. 等待操作，但同时在channel上监听更多的偶数.
4. 如果在现有操作完成之前接收到了新的偶数，要中止现在操作，并使用新的偶数重新开始.

```rust
async fn action(input: Option<i32>) -> Option<String> {
    // 如果输None则返回None
    // 也可以写成 let i = input?;`
    let i = match input {
        Some(input) => input,
        None => return None,
    };
    // 这里是异步逻辑
}

#[tokio::main]
async fn main() {
    let (mut tx, mut rx) = tokio::sync::mpsc::channel(128);
    
    let mut done = false;
    let operation = action(None);
    tokio::pin!(operation);
    
    tokio::spawn(async move {
        let _ = tx.send(1).await;
        let _ = tx.send(3).await;
        let _ = tx.send(2).await;
    });
    
    loop {
        tokio::select! {
            res = &mut operation, if !done => {
                done = true;

                if let Some(v) = res {
                    println!("GOT = {}", v);
                    return;
                }
            }
            Some(v) = rx.recv() => {
                if v % 2 == 0 {
                    // .set 是在 Pin 上的一个方法
                    operation.set(action(Some(v)));
                    done = false;
                }
            }
        }
    }
}
```
我们使用了与之前例子类似的策略. 异步函数在循环外部被调用并分配给`operation`变量. `operation`变量被固定. 循环同时在`operation`与通道接收在选择(select).

注意到，`action()`是怎样传入一个`Option<i32>`参数的，在我们收到第一个偶整数之前，我们必须实例化一些`operation`. 我们让`action()`传入`Option`并返回`Option`.
如果传入的是`None`就返回`None`. 第一次迭代，`operation`立即完成，并显示`None`.

这个示例使用了一些新语法. 第一个分支包含`, if !done`. 这是分支的前提. 在解释其工作原理之前，让我们看一下如果省略了前提条件会发生什么. 
省略`, if !done` 并运行示例会得到如下输出结果:

```text
thread 'main' panicked at '`async fn` resumed after completion', src/main.rs:1:55
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

当尝试在`operation`完成之后再去使用它，就会发生此错误. 通常，在使用`.await`时，等待的值会被消费. 在这个例子中我们在一个引用上等待.
这意味着`operation`完成之后它任然存在.

为了避免这种panic，如果`operation`完成了，我们必须注意禁用第一个分支. `done`变量用于跟踪`operation`是否完成. 一个`select!`分支可以包含
一个前提条件. `select!`在分支上等待之前该前提条件会被检查. 如果前提条件的评估结果是`false`，则禁用分支. `done`变量被初始化为`false`.
当`operation`完成后，`done`被设置为`true`. 下一次循环迭代将禁用`operation`分支. 当从channel中接收到偶数时，`operation`会被重置且
`done`再次被设置为 `false`.

## 每个任务的并发(Per-task concurrency)
`tokio::spawn` 与 `select!` 都可以运行并发异步操作. 但是用于运行并发操作的策略有所不同. `tokio::spawn` 函数传入一个异步操作并产生一个
新的任务去运行它. 任务是一个tokio运行时调度的对象. Tokio独立调度两个不同的任务. 它们可以在不同的操作系统线程上同时运行. 因此产生的任务与
产生的线程都有相同的限制: 不可借用.

`select!`宏能在同一个任务上同时运行所有分支. 因为`select!`宏上的所有分支被同一个任务执行，它们永远不会同时运行. `select!`宏的多路复用
异步操作也在单个任务上运行.


&larr; [深入异步(Async in depth)](AsyncInDepth.md)

&rarr; [流(Streams)](Streams.md)




