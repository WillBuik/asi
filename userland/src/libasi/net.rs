use std::{net::{TcpStream, SocketAddr}, os::fd::FromRawFd};

use libasi_interop::net::{NetError, ConnectRpcRequest, ConnectAddrs, LookupRpcRequest};

use super::rpc::rpc_call;

pub fn connect_tcp(addrs: &[SocketAddr]) -> Result<TcpStream, NetError> {
    let fd = rpc_call(&ConnectRpcRequest {
        target: ConnectAddrs::Tcp { addrs: addrs.to_vec() },
    })?;

    Ok(unsafe {
        TcpStream::from_raw_fd(fd)
    })
}

pub fn lookup(query: &str) -> Result<Vec<SocketAddr>, NetError> {
    rpc_call(&LookupRpcRequest {
        query: query.to_string(),
    })
}
