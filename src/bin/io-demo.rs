use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() {
    //read_to_end().await.unwrap();
    //write().await.unwrap();
    //write_to_all().await.unwrap();
    async_copy().await.unwrap();
}

/// 异步读read()
#[allow(dead_code)]
async fn read() -> io::Result<()>{
    let mut file = File::open("d:/foo.txt").await?;
    // 声明一个buffer
    let mut buffer =  [0;20];

    let n = file.read(&mut buffer).await?;

    println!("The bytes : {:?}", n);
    println!("The buffer content: {:?}", String::from_utf8(Vec::from(buffer)));

    Ok(())
}

/// 异步读取整个文件
#[allow(dead_code)]
async fn read_to_end() -> io::Result<()>{
    let mut f = File::open("d:/foo.txt").await?;
    let mut buffer = Vec::new();

    // 读取整个文件
    f.read_to_end(&mut buffer).await?;

    println!("file full content: {:?}", String::from_utf8(buffer));

    Ok(())
}

/// 读取文件中的所有内容到一个buffer中去
#[allow(dead_code)]
async fn read_to_all(buffer: &mut Vec<u8>) {
    let mut f = File::open("d:/foo.txt").await.unwrap();
    f.read_to_end(buffer).await.unwrap();
}

#[allow(dead_code)]
async fn write() -> io::Result<()> {
    let mut file = File::create("d:/create.txt").await?;

    // 写入一些字符串
    let n = file.write(b"some chars").await?;
    println!("write the first {} bytes of 'some bytes", n);

    Ok(())
}

#[allow(dead_code)]
async fn write_to_all() -> io::Result<()> {
    let mut file = File::create("d:/create1.txt").await?;

    // 从foo.txt文件中读取内容并写入到create1.txt中去
    let mut buffer:Vec<u8> = Vec::new();
    read_to_all(&mut buffer).await;
    //再将buffer中的数据写到新文件中去
    file.write_all(buffer.as_slice()).await?;
    Ok(())
}


#[allow(dead_code)]
async fn async_copy() -> io::Result<()> {
    // 实现将foo.txt reader中的内容不通过buffer直接copy到writer中去
    let mut target_file = File::create("d:/foo_copy.txt").await?; // 创建目标文件, 它也就是一个writer
    // 源文件
    let mut src_file = File::open("d:/foo.txt").await?;
    // tokio::io::copy 可以异步的将reader中的内容copy到writer中去
    io::copy(&mut src_file, &mut target_file).await?;

    Ok(())
}
