
use std::time::Duration;

use crate::utils::*;

pub struct ServerNamedPipeEvent<F>(ServerNamedPipe<F>, EventOwner);

impl<F> ServerNamedPipeEvent<F> {
    pub fn pipe_mut(&mut self) -> &mut ServerNamedPipe<F> {
        let Self(pipe, _) = self;
        pipe
    }
    
    pub fn pipe_ref(&self) -> &ServerNamedPipe<F> {
        let Self(pipe, _) = self;
        pipe
    }
    
    pub fn event(&self) -> Event {
        let Self(_, event) = self;
        event.duplicate()
    }
}

pub struct Server<F: 'static> {
    name: NamedPipePath,
    buffer_allocator: &'static F,
    windows_named_pipe_buffer_size: u32,
    client_default_timeout: Duration,
    pipes: Vec<ServerNamedPipeEvent<&'static F>>,
    new_event_sender: channel::Sender<Event>,
    connection_receiver: channel::Receiver<usize>,
    error_receiver: channel::Receiver<WindowsError>,
    interrupt_event: EventOwner,
    grow_event: EventOwner,
}

impl<F: 'static> Server<F> {
    pub fn new(
        name: NamedPipePath,
        buffer_allocator: &'static F,
        windows_named_pipe_buffer_size: u32,
        client_default_timeout: Duration
    ) -> WindowsResult<Self> {
        let interrupt_event = EventManager::register()?;
        let grow_event = EventManager::register()?;
        
        let mut events = vec![interrupt_event, grow_event];
        
        let (new_event_sender, new_event_receiver) = channel::Channel::new().unwrap();
        let (connection_sender, connection_receiver) = channel::Channel::new().unwrap();
        let (error_sender, error_receiver) = channel::Channel::new().unwrap();
        
        new_thread(move || {
            let non_pipe_events = events.len();
            let mut thread_interrupt = false;
            
            while !thread_interrupt {
                let mut grow = false;
                
                if let Err(error) = events.wait_signals_index(|i| {
                    if events[i] == interrupt_event {
                        thread_interrupt = true;
                    }
                    else if events[i] == grow_event {
                        grow = true;
                    }
                    else {
                        connection_sender.send(i - non_pipe_events);
                    }
                }) {
                    error_sender.send(error);
                }
                
                if grow {
                    unsafe { new_event_receiver.raw_buffer(|new_events| events.append(new_events)); }
                }
            }
        });
        
        Ok(Self {
            name: name.to_owned(),
            buffer_allocator,
            windows_named_pipe_buffer_size,
            client_default_timeout,
            pipes: Vec::new(),
            new_event_sender,
            connection_receiver,
            error_receiver,
            interrupt_event: EventOwner(interrupt_event),
            grow_event: EventOwner(grow_event),
        })
    }
    
    pub fn create_pipe(
        &mut self,
        windows_named_pipe_buffer_size: Option<u32>,
        client_default_timeout: Option<Duration>,
    ) -> WindowsResult<&mut ServerNamedPipeEvent<&'static F>> {
        let event = EventManager::register()?;
        let pipe = ServerNamedPipe::new(
            &self.name,
            windows_named_pipe_buffer_size.unwrap_or(self.windows_named_pipe_buffer_size),
            client_default_timeout.unwrap_or(self.client_default_timeout),
            LazyBuffer::Unbuffered(self.buffer_allocator),
        )?;
        
        self.pipes.push(ServerNamedPipeEvent(pipe, EventOwner(event)));
        self.new_event_sender.send(event);
        self.grow_event.duplicate().set()?;
        
        Ok(self.pipes.last_mut().unwrap())
    }
    
    pub fn create_pipes(
        &mut self,
        windows_named_pipe_buffer_size: Option<u32>,
        client_default_timeout: Option<Duration>,
        count: usize,
    ) -> WindowsResult<&mut [ServerNamedPipeEvent<&'static F>]> {
        for _ in 0..count {
            self.create_pipe(windows_named_pipe_buffer_size, client_default_timeout)?;
        }
        
        let len = self.pipes.len();
        
        Ok(&mut self.pipes[len - count..])
    }
    
    pub fn grow(
        &mut self,
        windows_named_pipe_buffer_size: Option<u32>,
        client_default_timeout: Option<Duration>,
    ) -> WindowsResult<&mut [ServerNamedPipeEvent<&'static F>]> {
        self.create_pipes(windows_named_pipe_buffer_size, client_default_timeout, self.pipes.len().min(1))
    }
    
    pub fn close(&self) -> WindowsResult<()> {
        self.interrupt_event.duplicate().set()?;
        
        for pipe in &self.pipes {
            pipe.pipe_ref().close()?;
        }
        
        Ok(())
    }
    
    pub fn get_connected_pipes(&self) -> Vec<usize> {
        self.connection_receiver.receive_all()
    }
    
    pub fn get_thread_errors(&self) -> Vec<WindowsError> {
        self.error_receiver.receive_all()
    }
    
    pub fn pipes(&mut self) -> &mut [ServerNamedPipeEvent<&'static F>] {
        &mut self.pipes
    }
}
