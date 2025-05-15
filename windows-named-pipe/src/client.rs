
use std::{thread::JoinHandle, time::Duration, u32};

use windows::Win32::{Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_SEM_TIMEOUT, GENERIC_ACCESS_RIGHTS, GENERIC_READ, GENERIC_WRITE}, Storage::FileSystem::{CreateFileA, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE, OPEN_EXISTING}, System::Pipes::{WaitNamedPipeA, NMPWAIT_USE_DEFAULT_WAIT, NMPWAIT_WAIT_FOREVER}};

use crate::utils::*;

#[repr(transparent)]
#[derive(Debug)]
pub struct Client(HANDLE);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum NamedPipeCheck {
    Available,
    Unavailable,
    Busy,
}

impl Client {
    pub fn check_pipe(pipe_name: &NamedPipePath) -> WindowsResult<NamedPipeCheck> {
        unsafe {
            match WaitNamedPipeA(pipe_name.as_pcstr(), 0) {
                Ok(_) => Ok(NamedPipeCheck::Available),
                _ if GetLastError() == ERROR_FILE_NOT_FOUND => Ok(NamedPipeCheck::Unavailable),
                _ if GetLastError() == ERROR_PIPE_BUSY => Ok(NamedPipeCheck::Busy),
                Err(error) => Err(error),
            }
        }
    }
    
    fn wait_pipe(pipe_name: &NamedPipePath, timeout: u32) -> WindowsResult<Option<Self>> {
        unsafe {
            match WaitNamedPipeA(pipe_name.as_pcstr(), timeout) {
                Ok(_) => {
                    let GENERIC_ACCESS_RIGHTS(access) = GENERIC_READ | GENERIC_WRITE;
                    
                    Ok(Some(Self(CreateFileA(
                        pipe_name.as_pcstr(),
                        access,
                        FILE_SHARE_NONE,
                        None,
                        OPEN_EXISTING,
                        FILE_FLAG_OVERLAPPED,
                        None,
                    )?)))
                }
                _ if GetLastError() == ERROR_SEM_TIMEOUT => Ok(None),
                Err(error) => Err(error),
            }
        }
    }
    
    pub fn wait(pipe_name: &NamedPipePath) -> WindowsResult<Self> {
        Self::wait_pipe(pipe_name, NMPWAIT_WAIT_FOREVER).map(Option::unwrap)
    }
    
    pub fn try_wait(pipe_name: &NamedPipePath, timeout: Duration) -> WindowsResult<Option<Self>> {
        Self::wait_pipe(pipe_name, timeout.as_millis().try_into().unwrap_or(u32::MAX))
    }
    
    pub fn try_wait_default(pipe_name: &NamedPipePath) -> WindowsResult<Option<Self>> {
        Self::wait_pipe(pipe_name, NMPWAIT_USE_DEFAULT_WAIT)
    }
    
    pub fn wait_in_background(pipe_name: &NamedPipePath, callback: impl FnOnce(WindowsResult<Self>) + Send + 'static) -> JoinHandle<()> {
        let pipe_name = pipe_name.to_owned();
        new_thread(move || callback(Self::wait(&pipe_name)))
    }
    
    pub fn initialize(self, buffer: NamedPipeBuffer, runtime: impl NamedPipeRuntimeExecutor) -> WindowsResult<NamedPipe> {
        let Self(handle) = self;
        NamedPipe::new(handle, buffer, runtime)
    }
}
