
pub mod double_buffer;
pub mod io_buffer;

pub use double_buffer::*;
pub use io_buffer::*;

use crate::utils::*;

pub enum LazyBuffer<T, F> {
    Buffered(T),
    Unbuffered(F),
}

impl<T, F: FnOnce() -> T> LazyBuffer<T, F> {
    pub fn buffer(self) -> T {
        match self {
            LazyBuffer::Buffered(buffer) => buffer,
            LazyBuffer::Unbuffered(f) => f(),
        }
    }
}

pub struct NamedPipeBuffer {
    pub read: IoBuffer,
    pub write: IoBuffer,
    pub read_channel: channel::Channel<u8>,
    pub write_channel: channel::Channel<u8>,
}
