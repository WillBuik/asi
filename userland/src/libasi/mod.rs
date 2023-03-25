use libasi_interop::diagnostics::{HelloRpcRequest, PokeRpcRequest};

use self::rpc::rpc_call;

pub mod log;
pub mod net;
mod rpc;

pub fn hello(who: impl ToString) {
    rpc_call(&HelloRpcRequest {
        who: who.to_string(),
    })
}

pub fn poke() -> u64 {
    rpc_call(&PokeRpcRequest)
}
