
use windows::Win32::{
    Foundation::{CloseHandle, ERROR_IO_PENDING},
    Storage::FileSystem::{
        ReadFile,
        WriteFile,
    },
};

use crate::utils::*;

pub mod utils;

pub trait NamedPipeRuntimeExecutor: FnOnce(&mut NamedPipeRuntime) + Send + 'static {}
impl<T: FnOnce(&mut NamedPipeRuntime) + Send + 'static> NamedPipeRuntimeExecutor for T {}

pub struct NamedPipeRuntime {
    handle: HANDLE,
    buffer: NamedPipeBuffer,
    events: NamedPipeEvents,
    read_pending: bool,
    write_pending: bool,
}

unsafe impl Send for NamedPipeRuntime {}

#[derive(Debug, Default)]
pub struct WaitResult {
    pub read: Option<WindowsResult<usize>>,
    pub write: Option<WindowsResult<usize>>,
    pub data: bool,
    pub interrupt: bool,
}

impl NamedPipeRuntime {
    pub(crate) fn new(handle: HANDLE, buffer: NamedPipeBuffer, events: NamedPipeEvents) -> Self {
        Self {
            handle,
            buffer,
            events,
            read_pending: false,
            write_pending: false,
        }
    }
    
    pub(crate) fn destruct(self) -> NamedPipeBuffer {
        let Self { buffer, .. } = self;
        
        buffer
    }
    
    pub fn wait(&mut self) -> (WaitResult, Option<WindowsError>) {
        unsafe {
            let mut result = WaitResult { ..Default::default() };
            
            let events = [
                self.events.read(),
                if self.write_pending { self.events.write() } else { self.events.data() },
                self.events.interrupt(),
            ];
            
            let error = events.wait_signals_event(|event| {
                if event == self.events.read() {
                    let (_, read_overlapped) = self.buffer.read.as_ref();
                    result.read.replace(get_overlapped_result(self.handle, read_overlapped));
                    self.read_pending = false;
                }
                
                if event == self.events.write() {
                    let (_, write_overlapped) = self.buffer.write.as_ref();
                    result.write.replace(get_overlapped_result(self.handle, write_overlapped));
                    self.write_pending = false;
                }
                
                if event == self.events.data() {
                    result.data = true;
                }
                
                if event == self.events.interrupt() {
                    result.interrupt = true;
                }
            }).err();
            
            (result, error)
        }
    }
    
    pub fn is_reading(&self) -> bool {
        self.read_pending
    }
    
    pub fn is_writing(&self) -> bool {
        self.write_pending
    }
    
    // returns true if there is no ongoing read operation
    pub fn read(&mut self) -> WindowsResult<bool> {
        unsafe {
            if self.read_pending { Ok(false) }
            else {
                self.buffer.read.set_event(self.events.read());
                
                let (buffer, overlapped) = self.buffer.read.as_mut();
                
                match ReadFile(self.handle, Some(buffer), None, Some(overlapped)) {
                    Ok(()) => self.events.read().set()?,
                    Err(_) if GetLastError() == ERROR_IO_PENDING => {}
                    Err(error) => Err(error)?,
                }
                
                self.read_pending = true;
                
                Ok(true)
            }
        }
    }
    
    // returns true if there is no ongoing write operation
    pub fn write(&mut self, len: usize) -> WindowsResult<bool> {
        unsafe {
            if self.write_buf().is_none_or(|buffer| !(1..buffer.len()).contains(&len)) { Ok(false) }
            else {
                self.buffer.write.set_event(self.events.write());
                
                let (buffer, overlapped) = self.buffer.write.as_ref();
                
                match WriteFile(self.handle, Some(&buffer[..len]), None, Some(overlapped)) {
                    Ok(()) => self.events.write().set()?,
                    Err(_) if GetLastError() == ERROR_IO_PENDING => {}
                    Err(error) => Err(error)?,
                }
                
                self.write_pending = true;
                
                Ok(true)
            }
        }
    }
    
    // returns the read buffer if there is no ongoing read operation
    pub fn read_buf(&self) -> Option<&[u8]> {
        (!self.read_pending).then(|| {
            let (buffer, _) = unsafe { self.buffer.read.as_ref() };
            
            buffer
        })
    }
    
    // returns the write buffer if there is no ongoing write operation
    pub fn write_buf(&mut self) -> Option<&mut [u8]> {
        (!self.write_pending).then(|| {
            let (buffer, _) = unsafe { self.buffer.write.as_mut() };
            
            buffer
        })
    }
    
    // returns true if there is no ongoing write operation
    pub fn receive(&mut self, f: impl FnOnce(&channel::Receiver<u8>, &mut [u8])) -> bool {
        if self.write_pending { false }
        else {
            unsafe {
                let (buffer, _) = self.buffer.write.as_mut();
                f(self.buffer.write_channel.receiver(), buffer);
                true
            }
        }
    }
    
    // returns true if there is no ongoing read operation
    pub fn send(&self, f: impl FnOnce(&channel::Sender<u8>, &[u8])) -> bool {
        if self.read_pending { false }
        else {
            unsafe {
                let (buffer, _) = self.buffer.read.as_ref();
                f(self.buffer.read_channel.sender(), buffer);
                true
            }
        }
    }
    
    pub unsafe fn close(&self) -> WindowsResult<()> {
        unsafe { CloseHandle(self.handle) }
    }
}
