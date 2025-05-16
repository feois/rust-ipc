
use crate::utils::*;

fn write(runtime: &mut NamedPipeRuntime) -> WindowsResult<bool> {
    let mut len = 0;
    
    runtime.receive(|receiver, bytes| {
        unsafe {
            receiver.raw_buffer(|buffer| {
                len = buffer.len().min(bytes.len());
                
                bytes[..len].copy_from_slice(&buffer[..len]);
            });
        }
    });
    
    runtime.write(len)
}

fn reference_implementation(runtime: &mut NamedPipeRuntime) -> WindowsResult<()> {
    runtime.read()?;
    
    loop {
        let (wait_result, error) = runtime.wait();
        
        if let Some(error) = error {
            Err(error)?;
        }
        
        if wait_result.interrupt {
            break;
        }
        
        if let Some(read_len) = wait_result.read {
            let read_len = read_len?;
            
            runtime.send(|sender, bytes| {
                unsafe { sender.raw_buffer(|buffer| buffer.extend(&bytes[..read_len])); }
            });
            
            runtime.read()?;
        }
        
        if let Some(write_len) = wait_result.write {
            let write_len = write_len?;
            
            runtime.receive(|receiver, _| unsafe { receiver.raw_buffer(|buffer| { buffer.drain(..write_len); }); });
            
            if !write(runtime)? {
                runtime.events.data().reset()?;
            }
        }
        
        if wait_result.data {
            write(runtime)?;
        }
    }
    
    Ok(())
}

pub fn runtime_reference_implementation(error_handler: impl FnOnce(WindowsError) + Send + 'static) -> impl NamedPipeRuntimeExecutor {
    |runtime| {
        if let Err(error) = reference_implementation(runtime) {
            error_handler(error)
        }
    }
}
