
use std::thread::JoinHandle;

use crate::utils::*;

#[derive(Debug, PartialEq, Eq)]
pub enum ReadLineResult {
    InvalidUtf8,
    Empty,
    NotALine,
    Line(String),
}

#[derive(Clone, Copy, Debug)]
pub struct NamedPipeEvents([Event; 4]);

impl NamedPipeEvents {
    pub fn register() -> WindowsResult<Self> {
        Ok(Self(EventManager::register_n()?))
    }
    
    pub fn unregister(self) {
        let Self(events) = self;
        
        events.into_iter().for_each(EventManager::unregister);
    }
    
    pub fn read(self) -> Event {
        let Self(events) = self;
        events[0]
    }
    
    pub fn write(self) -> Event {
        let Self(events) = self;
        events[1]
    }
    
    pub fn data(self) -> Event {
        let Self(events) = self;
        events[2]
    }
    
    pub fn interrupt(self) -> Event {
        let Self(events) = self;
        events[3]
    }
}

#[derive(Debug)]
struct NamedPipeEventsOwner(NamedPipeEvents);

impl Drop for NamedPipeEventsOwner {
    fn drop(&mut self) {
        let NamedPipeEventsOwner(events) = self;
        
        events.unregister();
    }
}

pub struct NamedPipe {
    thread: JoinHandle<NamedPipeBuffer>,
    write_sender: channel::Sender<u8>,
    read_receiver: channel::Receiver<u8>,
    events: NamedPipeEvents,
    #[allow(dead_code)]
    events_owner: NamedPipeEventsOwner, // for auto unregistering via drop
}

impl NamedPipe {
    pub fn new(handle: HANDLE, buffer: NamedPipeBuffer, executor: impl NamedPipeRuntimeExecutor) -> WindowsResult<Self> {
        let write_sender = unsafe { channel::clone_sender(buffer.write_channel.sender()) }; // reversed
        let read_receiver = unsafe { channel::clone_receiver(buffer.read_channel.receiver()) };
        let events = NamedPipeEvents::register()?;
        
        let mut runtime = NamedPipeRuntime::new(
            handle,
            buffer,
            events.clone(),
        );
        
        Ok(Self {
            thread: new_thread(move || {
                executor(&mut runtime);
                
                runtime.destruct()
            }),
            write_sender,
            read_receiver,
            events_owner: NamedPipeEventsOwner(events.clone()),
            events,
        })
    }
    
    pub fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }
    
    pub fn join(self) -> Result<NamedPipeBuffer, Box<dyn std::any::Any + Send + 'static>> {
        let Self { thread, .. } = self;
        
        thread.join()
    }
    
    pub fn flush(&self) {
        self.read_receiver.flush();
        self.write_sender.flush();
    }
    
    pub fn read(&self) -> Vec<u8> {
        self.read_receiver.receive_all()
    }
    
    pub fn read_line(&self) -> ReadLineResult {
        let mut result = ReadLineResult::Empty;
        
        unsafe {
            self.read_receiver.raw_buffer(|buffer| {
                
                result = if let Some(s) = buffer.utf8_chunks().next() {
                    if s.invalid().is_empty() {
                        let s = s.valid();
                        
                        if s.contains('\n') {
                            let s = s.lines().next().unwrap().to_owned();
                            
                            buffer.drain(..=s.len());
                            
                            ReadLineResult::Line(s)
                        }
                        else {
                            ReadLineResult::NotALine
                        }
                    }
                    else {
                        ReadLineResult::InvalidUtf8
                    }
                }
                else {
                    ReadLineResult::Empty
                }
            });
        }
        
        result
    }
    
    pub fn read_invalid_utf8(&self) -> Option<Vec<u8>> {
        let mut result = None;
        
        unsafe {
            self.read_receiver.raw_buffer(|buffer| {
                result = buffer.utf8_chunks().next()
                    .map(|s| s.invalid())
                    .and_then(|s| s.is_empty().then(|| s.to_owned()));
                
                if let Some(s) = &result {
                    buffer.drain(..s.len());
                }
            });
        }
        
        result
    }
    
    pub fn write(&self, bytes: &[u8]) -> WindowsResult<()> {
        unsafe {
            self.write_sender.raw_buffer(|vec| vec.extend(bytes));
            self.events.data().set()
        }
    }
    
    pub fn write_line(&self, s: &str) -> WindowsResult<()> {
        unsafe {
            self.write_sender.raw_buffer(|vec| {
                vec.extend(s.bytes());
                vec.push(b'\n');
            });
            self.events.data().set()
        }
    }
    
    pub fn interrupt(&self) -> WindowsResult<()> {
        self.events.interrupt().set()
    }
}
