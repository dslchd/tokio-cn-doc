use tokio::task;

#[tokio::main]
async fn main() {
    let v = vec![1,2,4];

    task::spawn(async move {
        println!("Here's a vec: {:?}", v);
    });
}