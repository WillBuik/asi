use std::{io::{self, Read, Write}, sync::{mpsc, Arc, atomic::{AtomicBool, Ordering}}, path::PathBuf, fs, thread::{JoinHandle, self}, time::Duration, path::Path};

use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian};
#[cfg(windows)]
use uds_windows::{UnixStream, UnixListener};
#[cfg(unix)]
use std::os::unix::net::{UnixStream, UnixListener};

pub type Error = io::Error;

/// Helper to delete a Unix socket file on drop.
struct SocketCleanup {
    path: PathBuf,
}

impl Drop for SocketCleanup {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            log::error!("Failed to clean up socket file '{}': {}", self.path.to_string_lossy(), err)
        }
    }
}

pub enum ClientRequest {
    Version,
    Shutdown,
    Run {
        binary: Vec<u8>,
    },
}

/// In-flight request from the control server.
pub struct InFlightRequest {
    request: ClientRequest,
    responder: Option<oneshot::Sender<Result<Vec<u8>, String>>>
}

impl InFlightRequest {
    /// Return the request data.
    pub fn request(&self) -> &ClientRequest{
        &self.request
    }

    /// Send a response to the requester.
    pub fn respond(self, response: Result<Vec<u8>, String>) {
        if let Some(responder) = self.responder {
            if let Err(_) = responder.send(response) {
                // This may fail if the requester has hung up.
                log::warn!("Requester hung up before response could be sent")
            }
        }
    }
}

/// Blocking, Unix-domain-socket control server for an a-Si host.
pub struct UdsControlServer {
    request_recv: mpsc::Receiver<InFlightRequest>,
    listener_thread: JoinHandle<()>,
    shutdown: Arc<AtomicBool>,
    _socket_cleanup: SocketCleanup,
}

impl UdsControlServer {
    const POLL_RATE: Duration = Duration::from_millis(50);
    const MAX_PAYLOAD: u64 = 1024*2024*50;

    /// Start the control server on a Unix socket at `path`.
    /// 
    /// If `handle_sigint` is true, the control server will trap termination
    /// signals and generate a shutdown request.
    pub fn start(path: impl AsRef<Path>, handle_sigint: bool) -> Result<Self, Error> {
        let (request_send, request_recv) = mpsc::channel();

        let listener = UnixListener::bind(&path)?;
        let socket_cleanup = SocketCleanup {
            path: path.as_ref().to_path_buf(),
        };
        listener.set_nonblocking(true)?;

        if handle_sigint {
            let request_send_term = request_send.clone();
            if let Err(err) = ctrlc::set_handler(move || {
                let shutdown_request = InFlightRequest {
                    request: ClientRequest::Shutdown,
                    responder: None,
                };
                if let Err(_) = request_send_term.send(shutdown_request) {
                    log::error!("Termination handler could not send shutdown request, killing host");
                    std::process::exit(-1);
                }
            }) {
                return Err(io::Error::new(io::ErrorKind::Other, err));
            }
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_listener = shutdown.clone();

        let listener_thread = thread::spawn(move || {
            Self::listener_thread(listener, request_send, shutdown_listener);
        });

        Ok(Self {
            request_recv,
            listener_thread,
            shutdown,
            _socket_cleanup: socket_cleanup,
        })
    }

    /// Wait for an incoming request.
    pub fn wait_request(&mut self) -> Result<InFlightRequest, Error> {
        match self.request_recv.recv() {
            Ok(req) => Ok(req),
            Err(err) => {
                Err(io::Error::new(io::ErrorKind::Other, err))
            },
        }
    }

    /// Shutdown the control server.
    pub fn shutdown(self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Err(_) = self.listener_thread.join() {
            log::error!("Failed to join listener thread");
        }
    }

    fn listener_thread(listener: UnixListener, sender: mpsc::Sender<InFlightRequest>, shutdown: Arc<AtomicBool>) {
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let sender_clone = sender.clone();
                    thread::spawn(move || {
                        if let Err(err) = Self::request_thread(stream, sender_clone) {
                            log::error!("Request handler error: {}", err);
                        }
                    });
                },
                Err(err) => {
                    if err.kind() != io::ErrorKind::WouldBlock {
                        log::error!("Listener error: {}", err);
                        return;
                    }
                },
            }

            if shutdown.load(Ordering::SeqCst) {
                break;
            }
            thread::sleep(Self::POLL_RATE);
        }
    }

    fn request_thread(mut stream: UnixStream, sender: mpsc::Sender<InFlightRequest>) -> Result<(), Error> {
        stream.set_nonblocking(false)?;

        let mut magic = [0u8;6];
        stream.read_exact(&mut magic)?;
        if &magic != b"aSiCLI" {
            log::debug!("Request has bad magic, dropping");
            return Ok(())
        }

        let op = stream.read_u8()?;

        let mut payloads = vec![];
        let mut total_payload = 0;
        let payload_count = stream.read_u8()?;
        for _ in 0..payload_count {
            let payload_len = stream.read_u64::<LittleEndian>()?;

            total_payload += payload_len;
            if total_payload > Self::MAX_PAYLOAD {
                return Err(io::Error::new(io::ErrorKind::Other, "payloads too big"));
            }

            let mut payload = [0u8].repeat(payload_len as usize);
            stream.read_exact(&mut payload)?;
            payloads.push(payload);
        }

        let (response_send, response_recv) = oneshot::channel();
        
        let request = match op {
            0 => {
                let request = InFlightRequest {
                    request: ClientRequest::Version,
                    responder: Some(response_send),
                };
                request
            },
            1 => {
                let request = InFlightRequest {
                    request: ClientRequest::Shutdown,
                    responder: Some(response_send),
                };
                request
            },
            2 => {
                if payload_count != 1 {
                    return Err(io::Error::new(io::ErrorKind::Other, "run requires one payload"));
                }
                let request = InFlightRequest {
                    request: ClientRequest::Run {
                        binary: payloads.pop().expect("has payload"),
                    },
                    responder: Some(response_send),
                };
                request
            },
            _ => {
                return Err(io::Error::new(io::ErrorKind::Other, "bad op"));
            }
        };

        if let Err(_) = sender.send(request) {
            return Err(io::Error::new(io::ErrorKind::Other, "server request recviever closed"));
        }

        let Ok(response) = response_recv.recv() else {
            return Err(io::Error::new(io::ErrorKind::Other, "response oneshot dropped without message"));
        };

        match response {
            Ok(buffer) => {
                stream.write_u8(0)?;
                stream.write_all(&buffer)?;
            },
            Err(message) => {
                stream.write_u8(1)?;
                stream.write_all(message.as_bytes())?;
            },
        }

        Ok(())
    }
}
