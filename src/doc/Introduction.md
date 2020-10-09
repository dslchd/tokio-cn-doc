## 指南(Tutorial)
这篇教程将一步步带你了解构建 [Redis](https://redis.io/) 客户端与服务端的过程. 我们将从使用Rust异步编程的基础开始. 我们将实现一个Redis
命令的子集并且获得关于Tokio的全方位的了解.

## 迷你-Redis(Mini-Redis)
你将在本教程中构建的工程可以在 [Mini-Redis on GitHub](https://github.com/tokio-rs/mini-redis) 上获得. Mini-Redis被设计的主要目的是
为了学习tokio,这种方式备受好评, 但这也意味着Mini-Redis缺少你想要的真实Redis库中的一些功能与特性. 你可以在这里 [crates.io](https://crates.io/)
上找到生产级可用Redis库.

我们将在本教程中直接使用Mini-Redis. 这使得我们在教程的后面实现部分之前,可以使用Mini-Redis的部分功能.

## 获取帮助(Getting Helping)
在任何时候, 如果你的学习被卡住, 你总能在 [Discord](https://discord.gg/tokio) 或 [Github discussions](https://github.com/tokio-rs/tokio/discussions) 上得到帮助.
不必担心问一些"初学者"问题. 我们都是从某个地方开始的,并且乐于提供帮助.

## 先决条件(Prerequisites)
读者应该已经熟悉了 [Rust](https://rust-lang.org/) . [Rust book](https://doc.rust-lang.org/book/) 是入门的绝佳资源.

虽然不是必须的, 但是你有Rust标准库或其它语言编写网络代码的一些经验的话,那将会有所帮助.

Redis相关的知识不是必要的.

## Rust
在我们开始之前, 你应该确保你已安装了 [Rust](https://www.rust-lang.org/tools/install) 工具链并做好了准备. 如果你没有做好这些, 使用
[rustup](https://rustup.rs/) 来安装是一个很好的方式.

本教程需要的最小Rust版本是 `1.39.0` , 但是推荐最好是最近的 Rust stable 版本.

为了检查Rust已经安装到你的电脑上, 执行如下命令查看:

```shell script
rustc --version
```
你应该会看到像 `rustc 1.43.1 (8d69840ab 2020-05-04)` 这样的输出.

## 迷你Redis服务(Mini-Redis server)
接下来安装Mini-Redis 服务. 它被用来测试我们即将要构建的客户端.
```shell script
cargo install mini-redis
```
执行下面的命令确保服务启动与安装成功了:
```shell script
mini-redis-server
```
然后尝试使用 `mini-redis-cli` 来得到键 `foo` 的值:
```shell script
mini-redis-cli get foo
```
你应该会看了 `nil` .

## 准备开始(Ready to go)
现在一切都作好了准备. 去到下一章节你将编写你的第一个Rust异步应用.


