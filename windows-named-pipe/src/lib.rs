
pub mod channel;
pub mod buffer;
pub mod pipe;
pub mod runtime;
pub mod server_pipe;
pub mod server;
pub mod client;
pub mod event;

pub(crate) mod utils;

pub mod prelude {
    pub use crate::{
        path::*,
        channel,
        buffer::{IoBuffer, NamedPipeBuffer},
        pipe::{NamedPipe, NamedPipeEvents, ReadLineResult},
        runtime::*,
        utils::WindowsResult,
        event::Event,
    };
    
    pub mod server {
        pub use super::*;
        pub use crate::{server::*, server_pipe::*};
    }
    
    pub mod client {
        pub use super::*;
        pub use crate::client::*;
    }
}

pub mod path {
    use std::ffi::CString;

    use windows::core::PCSTR;

    #[repr(transparent)]
    #[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct NamedPipePath(CString);

    impl NamedPipePath {
        pub fn new(pipe_name: &str) -> Self {
            Self(CString::new(String::from("\\\\.\\pipe\\") + pipe_name).expect("Pipe name should not contain NUL!"))
        }
        
        pub unsafe fn as_pcstr(&self) -> PCSTR {
            let Self(s) = self;
            
            PCSTR(s.to_bytes_with_nul().as_ptr())
        }
    }
}
