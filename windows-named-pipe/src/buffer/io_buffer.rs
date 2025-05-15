
use crate::utils::*;

pub struct IoBuffer {
    overlapped: NonNull<OVERLAPPED>,
    buffer: NonNull<[u8]>,
}

impl IoBuffer {
    pub fn new(len: usize) -> Self {
        unsafe {
            Self {
                overlapped: init_zero(alloc()),
                buffer: init_zeroes(alloc_n(len)),
            }
        }
    }
    
    pub fn set_event(&self, event: Event) {
        unsafe {
            self.overlapped.write(OVERLAPPED { hEvent: event.handle(), ..Default::default() });
        }
    }
    
    pub unsafe fn as_ref(&self) -> (&[u8], *mut OVERLAPPED) {
        unsafe { (self.buffer.as_ref(), self.overlapped.as_ptr()) }
    }
    
    pub unsafe fn as_mut(&mut self) -> (&mut [u8], *mut OVERLAPPED) {
        unsafe { (self.buffer.as_mut(), self.overlapped.as_ptr()) }
    }
}

unsafe impl Send for IoBuffer {}

impl Drop for IoBuffer {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.buffer);
            dealloc(self.overlapped);
        }
    }
}
