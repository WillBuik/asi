use std::net::SocketAddr;

use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::{RpcRequest, AsiFd};

const NET_BASE: u32 = 4000;

#[derive(Error, Serialize, Deserialize, Debug)]
pub enum NetError {
    #[error("access denied")]
    AccessDenied,

    #[error("query returned no results")]
    NotFound,

    #[error("operation failed")]
    Failed,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BindRpcRequest {
    pub bind_addr: BindAddr,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum BindAddr {
    Tcp {
        addr: SocketAddr,
    }
}

impl RpcRequest for BindRpcRequest {
    type Response = Result<AsiFd, NetError>;
    const OP_CODE: u32 = NET_BASE + 1;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConnectRpcRequest {
    pub target: ConnectAddrs,
}

impl RpcRequest for ConnectRpcRequest {
    type Response = Result<AsiFd, NetError>;
    const OP_CODE: u32 = NET_BASE + 2;
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ConnectAddrs {
    Tcp {
        addrs: Vec<SocketAddr>,
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LookupRpcRequest {
    pub query: String,
}

impl RpcRequest for LookupRpcRequest {
    type Response = Result<Vec<SocketAddr>, NetError>;
    const OP_CODE: u32 = NET_BASE + 3;
}
