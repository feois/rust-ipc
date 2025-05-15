
use std::time::Duration;

use crate::utils::*;

use windows::Win32::{
    Foundation::{
        CloseHandle,
        ERROR_IO_PENDING,
        ERROR_PIPE_CONNECTED,
    },
    Storage::FileSystem::{
        FlushFileBuffers,
        FILE_FLAG_OVERLAPPED,
        PIPE_ACCESS_DUPLEX,
    },
    System::Pipes::{
        ConnectNamedPipe,
        CreateNamedPipeA,
        DisconnectNamedPipe,
        PIPE_READMODE_BYTE,
        PIPE_TYPE_BYTE,
        PIPE_UNLIMITED_INSTANCES,
    },
};

pub enum ServerNamedPipeStatus {
    None, // only used internally
    Idle, // unconnected and not connecting
    Pending, // unconnected and connecting
    Connected(NamedPipe),
    Disconnected,
    ThreadPanic(Box<dyn std::any::Any + Send + 'static>), // contains error if thread panics
}

pub struct ServerNamedPipe<F> {
    handle: HANDLE,
    overlapped: NonNull<OVERLAPPED>,
    buffer: Option<LazyBuffer<NamedPipeBuffer, F>>,
    status: ServerNamedPipeStatus,
}

impl<F> ServerNamedPipe<F> {
    pub fn new(
        pipe_name: &NamedPipePath,
        windows_named_pipe_buffer_size: u32,
        client_default_timeout: Duration,
        pipe_buffer: LazyBuffer<NamedPipeBuffer, F>
    ) -> WindowsResult<Self> {
        unsafe {
            let handle = CreateNamedPipeA(
                pipe_name.as_pcstr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE,
                PIPE_UNLIMITED_INSTANCES,
                windows_named_pipe_buffer_size,
                windows_named_pipe_buffer_size,
                client_default_timeout.as_millis() as u32,
                None
            )?;
            
            Ok(Self {
                handle,
                overlapped: init_zero(alloc()),
                buffer: Some(pipe_buffer),
                status: ServerNamedPipeStatus::Idle,
            })
        }
    }
    
    fn connect(&self, event: Event) -> WindowsResult<bool> {
        unsafe {
            self.overlapped.write(OVERLAPPED { hEvent: event.handle(), ..Default::default() });
            
            match ConnectNamedPipe(self.handle, Some(self.overlapped.as_ptr())) {
                Ok(()) => Ok(true),
                Err(error) => match GetLastError() {
                    ERROR_IO_PENDING => Ok(false),
                    ERROR_PIPE_CONNECTED => Ok(true),
                    _ => Err(error),
                }
            }
        }
    }
    
    pub fn start_connecting(&mut self, event: Event) -> WindowsResult<()> {
        if let &ServerNamedPipeStatus::Idle = &self.status {
            if self.connect(event)? {
                event.set()?;
            }
            
            self.status = ServerNamedPipeStatus::Pending;
        }
        
        Ok(())
    }
    
    pub fn notify_connection(&mut self, runtime: impl NamedPipeRuntimeExecutor) -> WindowsResult<()> where F: FnOnce() -> NamedPipeBuffer {
        if let &ServerNamedPipeStatus::Pending = &self.status {
            self.status = ServerNamedPipeStatus::Connected(NamedPipe::new(self.handle, self.buffer.take().unwrap().buffer(), runtime)?);
        }
        
        Ok(())
    }
    
    pub fn update_status(&mut self) -> &ServerNamedPipeStatus {
        self.status = match std::mem::replace(&mut self.status, ServerNamedPipeStatus::None) {
            ServerNamedPipeStatus::None => unreachable!(),
            ServerNamedPipeStatus::Connected(pipe) if pipe.is_finished() => match pipe.join() {
                Ok(buffer) => {
                    self.buffer.replace(LazyBuffer::Buffered(buffer));
                    ServerNamedPipeStatus::Disconnected
                }
                Err(error) => ServerNamedPipeStatus::ThreadPanic(error),
            }
            status => status,
        };
        
        &self.status
    }
    
    // does not update status
    pub fn disconnect(&self) -> WindowsResult<()> {
        if let ServerNamedPipeStatus::Connected(_) = &self.status {
            unsafe {
                FlushFileBuffers(self.handle)?;
                
                DisconnectNamedPipe(self.handle)
            }
        }
        else {
            Ok(())
        }
    }
    
    pub fn close(&self) -> WindowsResult<()> {
        self.disconnect()?;
        
        unsafe { CloseHandle(self.handle) }
    }
    
    pub unsafe fn buffer(&mut self) -> &mut Option<LazyBuffer<NamedPipeBuffer, F>> {
        &mut self.buffer
    }
}

impl<F> Drop for ServerNamedPipe<F> {
    fn drop(&mut self) {
        unsafe { dealloc(self.overlapped) }
    }
}
