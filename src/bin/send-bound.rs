use tokio::task::yield_now;
use std::rc::Rc;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        // 通过　{} 作用域　使用　rc 在　.await前drop掉
        {
            let rc = Rc::new("hello");
            println!("{}", rc);
        }

        // rc 没有被使用了，所以当task返回到调度器时, 它不需要持续下去
        // 所以这个例子没问题
        yield_now().await; // 出让线程返回tokio 运行时调度器
    });

    // 但是如果改为下面这种就不能被编译
    tokio::spawn(async {
        let rc = Rc::new("hello");
        // 如果改为Arc就没有问题，因为　Arc 实现了　Send + Sync + 'static
        //let rc = Arc::new("hello");

        yield_now().await;

        // rc 在　.await之后还在使用, 它必须被保存在　task 的　状态中，这里没有保存所以不行
        println!("{}", rc);
    });
}
