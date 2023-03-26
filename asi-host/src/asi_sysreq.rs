use std::{cmp::min, mem::size_of, net::ToSocketAddrs};

use libasi_interop::{AsiRpcError, RpcRequest, diagnostics::{HelloRpcRequest, PokeRpcRequest, LogRpcRequest}, net::{ConnectRpcRequest, LookupRpcRequest, NetError, ConnectAddrs}};
use serde::Serialize;
use wasi_common::{WasiFile, file::FileType, Error, snapshots::preview_1::types::Errno};
pub struct AsiSysreqDevice {
    pending_response: Vec<u8>,
    count: u64,
}

impl AsiSysreqDevice {
    pub fn new() -> Self {
        Self {
            pending_response: Vec::new(),
            count: 0,
        }
    }

    fn hello(&mut self, hello: HelloRpcRequest) -> Result<<HelloRpcRequest as RpcRequest>::Response, AsiRpcError> {
        log::info!("SYSREQ Hello {}", hello.who);
        Ok(())
    }
    
    fn log(&mut self, record: LogRpcRequest) -> Result<<HelloRpcRequest as RpcRequest>::Response, AsiRpcError> {
        let level = match record.level {
            1 => log::Level::Error,
            2 => log::Level::Warn,
            3 => log::Level::Info,
            4 => log::Level::Debug,
            _ => log::Level::Trace,
        };
        let target = format!("GUEST:{}", record.target);

        let logger = log::logger();
        logger.log(&log::Record::builder()
            .level(level)
            .target(&target)
            .module_path(record.file.as_ref().map(|s| s.as_str()))
            .file(record.file.as_ref().map(|s| s.as_str()))
            .line(record.line)
            .args(format_args!("{}", record.body))
            .build()
        );

        Ok(())
    }

    fn poke(&mut self, _poke: PokeRpcRequest) -> Result<<PokeRpcRequest as RpcRequest>::Response, AsiRpcError> {
        self.count += 1;
        Ok(self.count)
    }

    fn net_connect(&mut self, connect: ConnectRpcRequest) -> Result<<ConnectRpcRequest as RpcRequest>::Response, AsiRpcError> {
        match connect.target {
            ConnectAddrs::Tcp { addrs: _ } => Err(AsiRpcError::BadRequest),
        }
    }

    fn net_lookup(&mut self, lookup: LookupRpcRequest) -> Result<<LookupRpcRequest as RpcRequest>::Response, AsiRpcError> {
        match lookup.query.to_socket_addrs() {
            Ok(addrs) => Ok(Ok(addrs.collect())),
            Err(_) => Ok(Err(NetError::Failed)),
        }
    }

    fn deserialize_request<T: RpcRequest> (buffer: &[u8]) -> Result<T, AsiRpcError> {
        match serde_json::from_slice(&buffer) {
            Ok(request) => Ok(request),
            Err(_) => Err(AsiRpcError::BadRequest),
        }
    }

    fn serialize_result<T: Serialize> (result: Result<T, AsiRpcError>) -> Vec<u8> {
        serde_json::to_vec(&result).unwrap_or_else(|_| vec![])
    }
}

#[async_trait::async_trait]
impl WasiFile for AsiSysreqDevice {
    fn as_any(&self) ->  &dyn std::any::Any {
        self
    }

    async fn get_filetype(&mut self) -> Result<FileType, Error> {
        Ok(FileType::Unknown)
    }

    async fn write_vectored<'a> (&mut self, bufs: &[std::io::IoSlice<'a>]) -> Result<u64, Error> {
        if self.pending_response.len() > 0 {
            // Guest wrote when it should have read.
            return Err(Errno::Inprogress.into())
        }

        let opcode = if bufs.len() == 1 && bufs[0].len() >= size_of::<u32>() {
            u32::from_le_bytes(bufs[0][0..size_of::<u32>()].try_into().expect("slice length of 4"))
        } else {
            // No support yet for vectored writes, these should never happen at the moment.
            return Err(Errno::Inval.into())
        };

        let request_buf = &bufs[0][size_of::<u32>()..];
        
        let resp = match opcode {
            HelloRpcRequest::OP_CODE => {
                Self::serialize_result(
                    Self::deserialize_request(request_buf).and_then(|req| self.hello(req)))
            },
            PokeRpcRequest::OP_CODE => {
                Self::serialize_result(
                    Self::deserialize_request(request_buf).and_then(|req| self.poke(req)))
            },
            LogRpcRequest::OP_CODE => {
                Self::serialize_result(
                    Self::deserialize_request(request_buf).and_then(|req| self.log(req)))
            },
            ConnectRpcRequest::OP_CODE => {
                Self::serialize_result(
                    Self::deserialize_request(request_buf).and_then(|req| self.net_connect(req)))
            },
            LookupRpcRequest::OP_CODE => {
                Self::serialize_result(
                    Self::deserialize_request(request_buf).and_then(|req| self.net_lookup(req)))
            },
            _ => {
                Self::serialize_result(Err::<(), _>(AsiRpcError::BadRequest))
            }
        };
        
        self.pending_response = resp;

        Ok(bufs[0].len() as u64)
    }

    async fn read_vectored<'a> (&mut self, bufs: &mut [std::io::IoSliceMut<'a>]) -> Result<u64, Error> {
        if bufs.len() != 1 {
            // No support yet for vectored writes, these should never happen at the moment.
            return Err(Errno::Inval.into());
        }

        let sz = min(bufs[0].len(), self.pending_response.len());
        bufs[0][0..sz].copy_from_slice(&self.pending_response[0..sz]);
        self.pending_response.drain(0..sz);

        Ok(sz as u64)
    }
}