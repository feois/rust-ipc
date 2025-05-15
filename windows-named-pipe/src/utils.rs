
use std::mem::MaybeUninit;

pub use std::ptr::NonNull;
pub use std::thread::spawn as new_thread;

pub use crate::{path::*, channel, event::*, buffer::*, pipe::*, runtime::*, server_pipe::*};

pub use windows::{
    core::{
        Error as WindowsError,
        Result as WindowsResult,
    },
    Win32::{
        Foundation::{
            GetLastError,
            HANDLE,
        },
        System::IO::{
            GetOverlappedResult,
            OVERLAPPED,
        },
    },
};

pub fn get_overlapped_result(handle: HANDLE, overlapped: *mut OVERLAPPED) -> WindowsResult<usize> {
    unsafe {
        let mut bytes = 0;
        
        GetOverlappedResult(handle, overlapped, &mut bytes, false)
                .map(|_| bytes as usize)
    }
}

pub unsafe fn assume_init<T>(pointer: NonNull<MaybeUninit<T>>) -> NonNull<T> {
    unsafe { NonNull::new_unchecked(pointer.as_ptr() as *mut T) }
}

pub unsafe fn assume_init_slice<T>(pointer: NonNull<[MaybeUninit<T>]>) -> NonNull<[T]> {
    unsafe { NonNull::new_unchecked(pointer.as_ptr() as *mut [T]) }
}

pub unsafe fn alloc<T>() -> NonNull<MaybeUninit<T>> {
    unsafe { NonNull::new_unchecked(Box::into_raw(Box::<T>::new_uninit())) }
}

pub unsafe fn alloc_n<T>(len: usize) -> NonNull<[MaybeUninit<T>]> {
    unsafe { NonNull::new_unchecked(Box::into_raw(Box::<[T]>::new_uninit_slice(len))) }
}

pub unsafe fn init_zero<T>(pointer: NonNull<MaybeUninit<T>>) -> NonNull<T> {
    unsafe {
        pointer.write(MaybeUninit::zeroed());
        
        assume_init(pointer)
    }
}

pub unsafe fn init_zeroes<T: Copy>(mut pointer: NonNull<[MaybeUninit<T>]>) -> NonNull<[T]> {
    unsafe {
        pointer.as_mut().fill(MaybeUninit::zeroed());
        
        assume_init_slice(pointer)
    }
}

pub unsafe fn dealloc<T: ?Sized>(pointer: NonNull<T>) {
    unsafe { std::mem::drop(Box::from_raw(pointer.as_ptr())); }
}
