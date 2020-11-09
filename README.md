# Tokio 中文文档
## 1.说明
Tokio 它是Rust语言的一种异步**运行时** 可以用来编写可靠，异步的Rust应用. 它有以下几个特点:
* 快速: Tokio是零成本抽象的，可以带给你接近裸机的性能.
* 可靠的: Tokio基于Rust语言的生命周期，类型系统，并发模型来减少bug和确保线程安全.
* 可扩展: Tokio有非常小的占用，并能处理背压(backpressure)和取消(cancellation)操作.

Tokio是一个事件驱动的非阻塞I/O平台，用于使用Rust编写异步应用. 在较高的层次上，它提供了几个主要的组件:
* 基于多线程与工作流窃取的 任务调度器 [scheduler](https://docs.rs/tokio/latest/tokio/runtime/index.html).
* 响应式的，基于操作系统的事件队列(比如，epoll, kqueue, IOCP, 等...).
* 异步的[TCP and UDP](https://docs.rs/tokio/latest/tokio/net/index.html) socket.

这些组件提供了用来构建异步应用所需要的运行时组件.

[官方原文指南](https://tokio.rs/tokio/tutorial).

[bin](src/bin) 目录下有一些可以参考的，基于官方文档的示例代码.

## 2.中文文档索引

### 指南
#### [介绍(Introduction)](doc/Introduction.md)
#### [你好 Tokio (Hello Tokio)](doc/HelloTokio.md)
#### [Spawning](doc/Spawning.md)
#### [共享状态(Shared state)](doc/SharedState.md)
#### [通道(Channels)](doc/Channels.md)
#### [I/O](doc/IO.md)
#### [帧(Framing)](doc/Framing.md)
#### [深入异步(Async in depth)](doc/AsyncInDepth.md)
#### [Select](doc/Select.md)
#### [流(Streams)](doc/Streams.md)
### [词汇表(Glossary)](doc/Glossary.md)
### [API文档(API documentation)](https://docs.rs/tokio)

## 3.其它
Tokio是一个非常值得学习的，Rust生态中的网络库. 有些类似 "Rust界的Netty" 的感觉，很多上层库，或包，或框架都是基于它(比如 Actix-web).
因此作为一名 _Rustaceans_ 学习与使用，或理解Tokio意义重大. 此**中文文档**是本人在学习Tokio后的 **"果实"** . 我顺便整理了出来.

由于水平有限，不敢妄言翻译的有多好，其中难免会有错误和遗漏，如果发现烦请一并指正. 望此文档能给同样对Tokio感兴趣的人的学习提供帮助.

我的其它翻译：[Actix-web 3.0 中文文档](https://github.com/dslchd/actix-web3-CN-doc).
