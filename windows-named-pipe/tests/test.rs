use std::{thread::scope, time::{Duration, Instant}};

use windows_named_pipe::{prelude::{client::*, server::*}, runtime::utils::runtime_reference_implementation};

fn pipe_runtime() -> impl NamedPipeRuntimeExecutor {
    runtime_reference_implementation(|_| ())
}

fn mainloop<T>(frame_length: Duration, mut f: impl FnMut() -> Option<T>) -> T {
    loop {
        let t = Instant::now();
        
        if let Some(r) = f() {
            return r;
        }
        
        let d = t.elapsed();
        
        if d < frame_length {
            spin_sleep::sleep(frame_length - d);
        }
    }
}

const IO_BUFFER_SIZE: usize = 65536;
const WINDOWS_BUFFER_SIZE: u32 = 65536;
const CLIENT_DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
const FPS: f64 = 120.;

#[test]
pub fn test() {
    let pipe_name = NamedPipePath::new("test");
    let frame_length = Duration::from_secs_f64(1. / FPS);
    let test_line = "test string";
    
    scope(|s| {
        {let pipe_name = pipe_name.clone();
        
        s.spawn(move || {
            let mut server = Server::new(
                pipe_name,
                &|| NamedPipeBuffer {
                    read: IoBuffer::new(IO_BUFFER_SIZE),
                    write: IoBuffer::new(IO_BUFFER_SIZE),
                    read_channel: channel::Channel::new(),
                    write_channel: channel::Channel::new(),
                },
                WINDOWS_BUFFER_SIZE,
                CLIENT_DEFAULT_TIMEOUT,
            ).expect("Failed to create server");
            
            let pipe = server.create_pipe(None, None).expect("Failed to create pipe");
            let event = pipe.event();
            
            let mut write = false;
            
            mainloop(frame_length, || {
                let connected = !server.get_connected_pipes().is_empty();
                let pipe = server.pipes()[0].pipe_mut();
                
                if connected {
                    pipe.notify_connection(pipe_runtime()).expect("Failed to connect pipe");
                }
                
                match pipe.update_status() {
                    &ServerNamedPipeStatus::None => unreachable!(),
                    &ServerNamedPipeStatus::Idle => pipe.start_connecting(event).expect("Failed to start connection"),
                    ServerNamedPipeStatus::Pending => {}
                    ServerNamedPipeStatus::Connected(connected_pipe) => {
                        if !write {
                            connected_pipe.write_line(test_line).expect("Failed to write line");
                            write = true;
                        }
                        
                        if let ReadLineResult::Line(_) = connected_pipe.read_line() {
                            return Some(()); // exit mainloop
                        }
                    }
                    &ServerNamedPipeStatus::Disconnected => panic!("Should have exited already!"),
                    ServerNamedPipeStatus::ThreadPanic(_error) => panic!("Thread poisoned"),
                }
                
                for error in server.get_thread_errors() { eprintln!("Error {}", error); }
                
                None
            });
            
            server.close().expect("Failed to close server");
        });}
        
        s.spawn(|| {
            while let NamedPipeCheck::Unavailable = Client::check_pipe(&pipe_name).expect("Failed to check pipe") {
                spin_sleep::sleep(Duration::from_secs(1));
            }
            
            let pipe = Client::wait(&pipe_name).expect("Failed to wait pipe").initialize(
                NamedPipeBuffer {
                    read: IoBuffer::new(IO_BUFFER_SIZE),
                    write: IoBuffer::new(IO_BUFFER_SIZE),
                    read_channel: channel::Channel::new(),
                    write_channel: channel::Channel::new(),
                },
                pipe_runtime(),
            ).expect("Failed to initialize pipe");
            
            mainloop::<()>(frame_length, || {
                match pipe.read_line() {
                    ReadLineResult::Line(line) => {
                        assert_eq!(line, test_line);
                        pipe.write_line("").expect("Failed to write line");
                    }
                    _ => {}
                }
                
                if pipe.is_finished() {
                    return Some(());
                }
                
                None
            });
        });
    });
}
