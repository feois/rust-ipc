use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct DoubleBuffer<T> {
    write: Mutex<Vec<T>>,
    read: Mutex<Vec<T>>,
}

impl<T> DoubleBuffer<T> {
    pub fn new() -> Self {
        Self { write: Mutex::new(Vec::new()), read: Mutex::new(Vec::new()) }
    }
    
    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self::new())
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        Self { write: Mutex::new(Vec::with_capacity(capacity)), read: Mutex::new(Vec::with_capacity(capacity)) }
    }
    
    pub fn flush(&self) {
        let ref mut write = self.write.lock().unwrap();
        let mut read = self.read.lock().unwrap();
        
        read.append(write);
    }
    
    pub fn try_flush(&self) -> bool {
        let Ok(ref mut write) = self.write.try_lock() else { return false };
        let Ok(mut read) = self.read.try_lock() else { return false };
        
        read.append(write);
        
        true
    }
    
    pub fn write(&self, f: impl FnOnce(&mut Vec<T>)) {
        if let Ok(ref mut write) = self.write.try_lock() {
            f(write);
            
            if let Ok(mut read) = self.read.try_lock() {
                read.append(write);
            }
        }
    }
    
    pub fn write_vec(&self, vec: &mut Vec<T>) {
        self.write(|t| t.append(vec));
    }
    
    pub fn push(&self, t: T) {
        self.write(|vec| vec.push(t));
    }
    
    pub fn read(&self, f: impl FnOnce(&mut Vec<T>)) {
        if let Ok(ref mut read) = self.read.try_lock() {
            f(read);
            
            if let Ok(ref mut write) = self.write.try_lock() {
                read.append(write);
            }
        }
    }
    
    pub fn read_vec(&self) -> Vec<T> {
        let mut vec = Vec::new();
        
        self.read(|t| vec.append(t));
        
        vec
    }
    
    pub fn pop(&self) -> Option<T> {
        let mut result = None;
        
        self.read(|vec| result = vec.pop());
        
        result
    }
    
    pub fn read_all(self) -> Vec<T> {
        let mut read = self.read.into_inner().unwrap();
        let mut write = self.write.into_inner().unwrap();
        
        read.append(&mut write);
        
        read
    }
}
