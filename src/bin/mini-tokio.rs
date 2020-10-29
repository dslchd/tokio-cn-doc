use std::collections::VecDeque;
use futures::task;

fn main() {}

struct MiniTokio {
    tasks: VecDeque<task>,
}