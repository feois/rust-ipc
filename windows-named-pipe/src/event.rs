
use std::sync::Mutex;

use windows::Win32::{Foundation::{CloseHandle, WAIT_EVENT, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT}, System::Threading::{CreateEventA, ResetEvent, SetEvent, WaitForMultipleObjects, WaitForSingleObject, INFINITE}};

use crate::utils::*;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Event(HANDLE);

unsafe impl Sync for Event {}
unsafe impl Send for Event {}

impl Event {
    pub unsafe fn new(handle: HANDLE) -> Self {
        Self(handle)
    }
    
    pub unsafe fn handle(self) -> HANDLE {
        let Event(handle) = self;
        
        handle
    }
    
    pub fn signal(self) -> WindowsResult<bool> {
        unsafe {
            match WaitForSingleObject(self.handle(), 0) {
                WAIT_OBJECT_0 => Ok(true),
                WAIT_TIMEOUT => Ok(false),
                WAIT_FAILED => Err(WindowsError::from_win32()),
                _ => unreachable!(),
            }
        }
    }
    
    pub fn set(self) -> WindowsResult<()> {
        unsafe { SetEvent(self.handle()) }
    }
    
    pub fn reset(self) -> WindowsResult<()> {
        unsafe { ResetEvent(self.handle()) }
    }
    
    pub unsafe fn null() -> Self {
        Self(HANDLE(std::ptr::null_mut()))
    }
}

pub trait EventPool {
    fn wait_signals_index(&self, f: impl FnMut(usize)) -> WindowsResult<()>;
    fn wait_signals_event(&self, f: impl FnMut(Event)) -> WindowsResult<()>;
}

impl<T: AsRef<[Event]>> EventPool for T {
    fn wait_signals_index(&self, mut f: impl FnMut(usize)) -> WindowsResult<()> {
        let event_slice = self.as_ref();
        let handle_slice = unsafe { std::slice::from_raw_parts(event_slice.as_ptr() as *const HANDLE, event_slice.len()) };
        
        let WAIT_EVENT(zero) = WAIT_OBJECT_0;
        
        unsafe {
            let mut code = match WaitForMultipleObjects(handle_slice, false, INFINITE) {
                WAIT_FAILED => Err(WindowsError::from_win32())?,
                WAIT_EVENT(code) => code,
            };
            
            loop {
                let index = (code - zero) as usize;
                
                ResetEvent(handle_slice[index])?;
                f(index);
                
                code = match WaitForMultipleObjects(handle_slice, false, 0) {
                    WAIT_FAILED => Err(WindowsError::from_win32())?,
                    WAIT_TIMEOUT => break,
                    WAIT_EVENT(code) => code,
                };
            }
        }
        
        Ok(())
    }
    
    fn wait_signals_event(&self, mut f: impl FnMut(Event)) -> WindowsResult<()> {
        self.wait_signals_index(|index| f(self.as_ref()[index]))
    }
}

pub struct EventManager;

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());

fn create_event() -> WindowsResult<Event> {
    unsafe { CreateEventA(None, true, false, None).map(Event) }
}

fn register(events: &mut Vec<Event>) -> WindowsResult<Event> {
    if let Some(event) = events.last() {
        event.reset()?;
        
        Ok(events.pop().unwrap())
    }
    else {
        create_event()
    }
}

fn register_n<const N: usize>(mut f: impl FnMut() -> WindowsResult<Event>) -> WindowsResult<[Event; N]> {
    let mut events = [unsafe { Event::null() }; N];
    
    for i in 0..N {
        events[i] = match f() {
            Ok(event) => event,
            Err(error) => {
                for i in 0..i {
                    EventManager::unregister(events[i]);
                }
                
                return Err(error);
            }
        }
    }
    
    return Ok(events);
}

#[allow(unused_must_use)]
fn try_lock_events<T>(f: impl FnOnce(Option<&mut Vec<Event>>) -> T) -> T {
    match EVENTS.try_lock() {
        Ok(mut vec) => f(Some(&mut vec)),
        Err(std::sync::TryLockError::WouldBlock) => f(None),
        poisoned => { poisoned.unwrap(); unreachable!() } // panic
    }
}

impl EventManager {
    pub fn register() -> WindowsResult<Event> {
        Self::try_register().and_then(|event| event.map_or_else(|| create_event(), Ok))
    }
    
    pub fn register_n<const N: usize>() -> WindowsResult<[Event; N]> {
        try_lock_events(|events| match events {
            Some(events) => register_n(|| register(events)),
            None => register_n(create_event),
        })
    }
    
    pub fn register_blocking() -> WindowsResult<Event> {
        register(&mut EVENTS.lock().unwrap())
    }
    
    pub fn register_n_blocking<const N: usize>() -> WindowsResult<[Event; N]> {
        let mut events = EVENTS.lock().unwrap();
        register_n(|| register(&mut events))
    }
    
    pub fn try_register() -> WindowsResult<Option<Event>> {
        try_lock_events(|events| events.map(|events| register(events))).transpose()
    }
    
    pub fn try_register_n<const N: usize>() -> WindowsResult<Option<[Event; N]>> {
        try_lock_events(|events| events.map(|events| register_n(|| register(events)))).transpose()
    }
    
    pub fn unregister(event: Event) {
        EVENTS.lock().unwrap().push(event)
    }
    
    pub fn close_events() -> Result<(), (WindowsError, HANDLE)> {
        unsafe {
            let mut events = EVENTS.lock().unwrap();
            
            while let Some(Event(event)) = events.pop() {
                match CloseHandle(event) {
                    Ok(()) => {}
                    Err(error) => return Err((error, event))
                }
            }
            
            Ok(())
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[repr(transparent)]
pub struct EventOwner(pub Event);

impl EventOwner {
    pub fn duplicate(&self) -> Event {
        let Self(event) = self;
        
        *event
    }
}

impl Drop for EventOwner {
    fn drop(&mut self) {
        let Self(event) = self;
        
        if *event != unsafe { Event::null() } {
            EventManager::unregister(*event);
        }
    }
}
