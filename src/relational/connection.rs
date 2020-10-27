use tokio::net::TcpStream;
use mini_redis::{Frame, Result};
use bytes::BytesMut;
use tokio::io::AsyncReadExt;

pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            // 默认分配4kb容量给buffer
            buffer: BytesMut::with_capacity(4 * 1024),
        }
    }
    /// 从链接中读取一个帧，如果EOF到就返回 None
    pub async fn read_frame(&mut self) -> Result<Option<Frame>>{
        loop {
            // 尝试从缓冲区中解析一个帧，如果buffer中有足够的数据那么帧就返回
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            // 如果没有足够的数据读取到一个帧中,那么尝试从socket中读取更多的数据
            // 如果成功了，一定数量的字节被返回, 0 表时到了流的末尾了.
            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                // 远程关闭了链接, 为了彻底关闭,读缓冲区中应该没有数据了,如果还存在数据那说明对等方在发送帧时关闭了socket
                return if self.buffer.is_empty() {
                    Ok(None)
                }else {
                    Err("connection reset by peer".into())
                }
            }

        }
    }

    /// 写一个帧到链接中
    pub async fn write_frame(&mut self, frame:&Frame) -> Result<()> {

    }

    fn parse_frame() -> Result<()>{

    }
}