use std::{fs::File, os::fd::{FromRawFd, RawFd}, io::{Write, Read, Cursor}, mem::size_of, cell::RefCell};

use libasi_interop::{RpcRequest, AsiRpcError};

struct AsiRpcGuestDevice {
    file: File,
    poisoned: bool,
}

impl AsiRpcGuestDevice {
    pub unsafe fn new_from_root() -> Self {
        let fd_str = match std::env::var("ASI_RPCROOT_FD") {
            Ok(str) => str,
            Err(_) => panic!("not running in an a-Si environment"),
        };

        let fd: RawFd = fd_str.parse().unwrap_or(-1);
        if fd < 0 {
            panic!("invalid a-Si RPC root device descriptor");
        }

        Self::new_from_fd(fd)
    }

    pub unsafe fn new_from_fd(fd: RawFd) -> Self {
        Self {
            file: File::from_raw_fd(fd),
            poisoned: false,
        }
    }

    pub fn call<T: RpcRequest> (&mut self, request: &T) -> Result<T::Response, AsiRpcError> {
        if self.poisoned {
            return Err(AsiRpcError::BadDescriptor);
        }

        let mut req_buffer = Cursor::new(T::OP_CODE.to_le_bytes().to_vec());
        req_buffer.set_position(size_of::<u32>() as u64);
        if serde_json::to_writer(&mut req_buffer, request).is_err() {
            return Err(AsiRpcError::BadRequest);
        }
        let req_buffer = req_buffer.into_inner();

        match self.file.write(&req_buffer) {
            Ok(sz) => {
                if req_buffer.len() != sz {
                    // Device must accept the entire write, the device is in an unknown state.
                    self.poisoned = true;
                    return Err(AsiRpcError::BadDescriptor);
                }
            },
            Err(_) => {
                // Device was not ready, the device is in an unknown state.
                self.poisoned = true;
                return Err(AsiRpcError::BadDescriptor);
            },
        }
        drop(req_buffer);

        let mut resp_buffer = Vec::new();
        if let Err(_) = self.file.read_to_end(&mut resp_buffer) {
            // An error was encountered when reading the response, the device is in an unknown state.
            self.poisoned = true;
            return Err(AsiRpcError::BadDescriptor);
        }

        match serde_json::from_slice(&resp_buffer) {
            Ok(response) => response,
            Err(_) => Err(AsiRpcError::BadResponse),
        }
    }

}

thread_local!(
    static THREAD_RPC_DEVICE: RefCell<AsiRpcGuestDevice> = unsafe {
        RefCell::new(AsiRpcGuestDevice::new_from_root())
    }
);

pub(super) fn rpc_call<T: RpcRequest> (request: &T) -> T::Response {
    let response_result = THREAD_RPC_DEVICE.with(|rpc_dev| {
        rpc_dev.borrow_mut().call(request)
    });
    match response_result {
        Ok(response) => response,
        Err(err) => panic!("a-Si RPC call ({}) failed: {}", T::OP_CODE, err),
    }
}
