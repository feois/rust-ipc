
use std::sync::Arc;

use crate::utils::*;

#[repr(transparent)]
#[derive(Debug)]
pub struct Sender<T>(Arc<DoubleBuffer<T>>);

#[repr(transparent)]
#[derive(Debug)]
pub struct Receiver<T>(Arc<DoubleBuffer<T>>);

#[repr(transparent)]
#[derive(Debug)]
pub struct UniqueReceiver<T>(DoubleBuffer<T>);

#[derive(Debug)]
pub struct Channel<T>(Sender<T>, Receiver<T>);

impl<T> Channel<T> {
    pub fn new() -> Self {
        let buffer = DoubleBuffer::new_arc();
        
        Self(Sender(buffer.clone()), Receiver(buffer))
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        let buffer = Arc::new(DoubleBuffer::with_capacity(capacity));
        
        Self(Sender(buffer.clone()), Receiver(buffer))
    }
    
    pub unsafe fn sender(&self) -> &Sender<T> {
        let Self(sender, _) = self;
        
        sender
    }
    
    pub unsafe fn receiver(&self) -> &Receiver<T> {
        let Self(_, receiver) = self;
        
        receiver
    }
    
    pub fn unwrap(self) -> (Sender<T>, Receiver<T>) {
        let Self(sender, receiver) = self;
        
        (sender, receiver)
    }
}

pub unsafe fn clone_sender<T>(sender: &Sender<T>) -> Sender<T> {
    let Sender(buffer) = sender;
    
    Sender(buffer.clone())
}

pub unsafe fn clone_receiver<T>(receiver: &Receiver<T>) -> Receiver<T> {
    let Receiver(buffer) = receiver;
    
    Receiver(buffer.clone())
}

impl<T> Sender<T> {
    fn buffer(&self) -> &DoubleBuffer<T> {
        let Self(buffer) = self;
        
        &buffer
    }
    
    pub fn flush(&self) {
        self.buffer().flush();
    }
    
    pub fn try_flush(&self) -> bool {
        self.buffer().try_flush()
    }
    
    pub fn send(&self, t: T) {
        self.buffer().push(t);
    }
    
    pub fn send_vec(&self, vec: &mut Vec<T>) {
        self.buffer().write_vec(vec);
    }
    
    pub unsafe fn raw_buffer(&self, f: impl FnOnce(&mut Vec<T>)) {
        self.buffer().write(f);
    }
}

impl<T> Receiver<T> {
    fn buffer(&self) -> &DoubleBuffer<T> {
        let Self(buffer) = self;
        
        &buffer
    }
    
    pub fn flush(&self) {
        self.buffer().flush();
    }
    
    pub fn try_flush(&self) -> bool {
        self.buffer().try_flush()
    }
    
    pub fn receive_latest(&self) -> Option<T> {
        self.buffer().pop()
    }
    
    pub fn receive_all(&self) -> Vec<T> {
        self.buffer().read_vec()
    }
    
    pub unsafe fn raw_buffer(&self, f: impl FnOnce(&mut Vec<T>)) {
        self.buffer().read(f);
    }
    
    pub fn unique(self) -> Result<UniqueReceiver<T>, Self> {
        let Receiver(buffer) = self;
        
        Arc::try_unwrap(buffer).map(UniqueReceiver).map_err(Receiver)
    }
}

impl<T> UniqueReceiver<T> {
    fn buffer(&self) -> &DoubleBuffer<T> {
        let Self(buffer) = self;
        
        &buffer
    }
    
    pub fn receive_latest(&self) -> Option<T> {
        self.buffer().pop()
    }
    
    pub fn receive_all(self) -> Vec<T> {
        let Self(buffer) = self;
        
        buffer.read_all()
    }
    
    pub unsafe fn raw_buffer(&self, f: impl FnOnce(&mut Vec<T>)) {
        self.buffer().read(f);
    }
}

