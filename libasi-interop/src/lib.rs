use serde::{Serialize, de::DeserializeOwned, Deserialize};
use thiserror::Error;

pub mod diagnostics;
pub mod net;

pub trait RpcRequest: Serialize + DeserializeOwned {
    type Response: Serialize + DeserializeOwned;

    const OP_CODE: u32;
}

#[derive(Error, Serialize, Deserialize, Debug)]
pub enum AsiRpcError {
    /// The underlying a-Si system request file desciptor is invalid or in a bad state.
    #[error("bad descriptor")]
    BadDescriptor,

    /// The a-Si host rejected the request.
    #[error("bad request")]
    BadRequest,

    /// The a-Si host send an invalid response.
    #[error("bad response")]
    BadResponse,
}

pub type AsiFd = i32;
