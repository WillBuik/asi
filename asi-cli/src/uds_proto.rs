
use std::{io::{Write, Read}, path::Path, vec};

use byteorder::{WriteBytesExt, LittleEndian, ReadBytesExt};

#[cfg(windows)]
use uds_windows::UnixStream;
#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("server error: {0}")]
    ServerError(String),

    #[error("protocol error: {0}")]
    ProtocolError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub enum ClientRequest<'a> {
    ServerVersion,
    Shutdown,
    Run {
        binary_data: &'a [u8]
    },
}

impl <'a> ClientRequest<'a> {
    fn op(&self) -> u8 {
        match self {
            ClientRequest::ServerVersion => 0,
            ClientRequest::Shutdown => 1,
            ClientRequest::Run {..} => 2,
        }
    }

    fn payloads(&self) -> Vec<&[u8]> {
        match self {
            ClientRequest::Run { binary_data } => vec![binary_data],
            _ => vec![],
        }
    }
}

pub struct AsiClient {
    stream: UnixStream,
}

impl AsiClient {
    pub fn new(socket_path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(Self {
            stream: UnixStream::connect(socket_path)?
        })
    }

    fn send_frame(&mut self, request: ClientRequest) -> Result<Vec<u8>, Error> {
        let payloads = request.payloads();
        if payloads.len() > u8::MAX as usize {
            panic!();
        }

        self.stream.write_all(b"aSiCLI")?;

        self.stream.write_u8(request.op())?;
        self.stream.write_u8(payloads.len() as u8)?;

        for payload in payloads {
            self.stream.write_u64::<LittleEndian>(payload.len() as u64)?;
            self.stream.write_all(payload)?;
        }

        let response = self.stream.read_u8()?;
        match response {
            0 => {

                let mut payload = vec![];
                self.stream.read_to_end(&mut payload)?;
                Ok(payload)
            },
            1 => {
                let mut message = vec![];
                self.stream.read_to_end(&mut message)?;
                Err(Error::ServerError(String::from_utf8_lossy(&message).to_string()))
            },
            _ => {
                Err(Error::ProtocolError("bad response code".to_string()))
            }
        }
    }

    pub fn version(mut self) -> Result<String, Error> {
        let version = self.send_frame(ClientRequest::ServerVersion)?;
        String::from_utf8(version).map_err(|_| Error::ProtocolError("version response not UTF-8".to_string()))
    }

    pub fn shutdown(mut self) -> Result<(), Error> {
        self.send_frame(ClientRequest::Shutdown)?;

        Ok(())
    }

    pub fn run(mut self, binary_data: &[u8]) -> Result<(), Error> {
        let request = ClientRequest::Run {
            binary_data
        };
        self.send_frame(request)?;

        Ok(())
    }
}