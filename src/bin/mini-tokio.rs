use std::collections::VecDeque;
use futures::{task, Future};
use std::pin::Pin;

fn main() {}

struct MiniTokio {
    tasks: VecDeque<task>,
}

type Task = Pin<Box<dyn Future<Output = ()> + Send>>;

impl MiniTokio {
    fn new() -> MiniTokio {
        MiniTokio {tasks: VecDeque::new()}
    }

    fn spawn<F>(&mut self, future: F)
    where F:Future<Output = ()> + Send + 'static,
    {
        self.tasks.push_back(Box::pin(future));
    }
}