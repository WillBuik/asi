use serde::{Serialize, Deserialize};

use crate::RpcRequest;

const DIAGNOSTICS_BASE: u32 = 10000;

#[derive(Serialize, Deserialize, Debug)]
pub struct HelloRpcRequest {
    pub who: String
}

impl RpcRequest for HelloRpcRequest {
    type Response = ();
    const OP_CODE: u32 = DIAGNOSTICS_BASE + 10;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PokeRpcRequest;

impl RpcRequest for PokeRpcRequest {
    type Response = u64;
    const OP_CODE: u32 = DIAGNOSTICS_BASE + 11;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LogRpcRequest {
    pub target: String,
    pub level: u32,
    pub body: String,
    pub module_path: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl RpcRequest for LogRpcRequest {
    type Response = ();
    const OP_CODE: u32 = DIAGNOSTICS_BASE + 200;
}