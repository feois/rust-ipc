
use crate::utils::*;

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
        
        if let Some(read) = wait_result.read {
            let read = read?;
            
            runtime.send(|sender, bytes| {
                unsafe { sender.raw_buffer(|buffer| buffer.extend(&bytes[..read])); }
            });
            
            runtime.read()?;
        }
        
        if let Some(write) = wait_result.write {
            let write = write?;
            
            runtime.receive(|receiver, _| unsafe { receiver.raw_buffer(|buffer| { buffer.drain(..write); }); });
        }
        
        if wait_result.data {
            let mut len = 0;
            
            runtime.receive(|receiver, bytes| {
                unsafe {
                    receiver.raw_buffer(|buffer| {
                        len = buffer.len().min(bytes.len());
                        
                        bytes[..len].copy_from_slice(&buffer[..len]);
                    });
                }
            });
            
            runtime.write(len)?;
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
