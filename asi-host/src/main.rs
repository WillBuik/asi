use std::{path::Path, thread::JoinHandle};

use asi_sysreq::AsiSysreqDevice;
use log::LevelFilter;
use wasi_common::{file::{FileType, FileCaps}, Error};
use wasmtime::{Engine, Store, Linker, Module};
use wasmtime_wasi::{WasiCtxBuilder, WasiFile};

use crate::uds_server::{UdsControlServer, ClientRequest};

pub mod asi_sysreq;
pub mod uds_server;

struct OutputHandler {

}

#[async_trait::async_trait]
impl WasiFile for OutputHandler {
    fn as_any(&self) ->  &dyn std::any::Any {
        self
    }

    async fn get_filetype(&mut self) -> Result<FileType, Error> {
        Ok(FileType::CharacterDevice)
    }

    async fn write_vectored<'a>(&mut self, bufs: &[std::io::IoSlice<'a>]) -> Result<u64, Error> {
        let mut len = 0;
        for buf in bufs {
            if let Ok(str) = std::str::from_utf8(&buf) {
                print!("{}", str);
            }
            len += buf.len();
        }
        //println!("wrote {} chars", len);
        Ok(len as u64)
    }
}

struct AsiBasicHost {
    engine: Engine,
    processes: Vec<JoinHandle<anyhow::Result<()>>>,
}

impl AsiBasicHost {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
            processes: Vec::new(),
        }
    }

    /*
    /// Start an aSi process from a module on the local disk.
    pub fn spawn_process_local(&mut self, wasi_module_path: impl AsRef<Path>) -> anyhow::Result<()> {
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;
        let wasi = WasiCtxBuilder::new().stdout(Box::new(OutputHandler{})).build();
        let mut store = Store::new(&self.engine, wasi);

        let module = Module::from_file(&self.engine, wasi_module_path)?;
        linker.module(&mut store, "", &module)?;

        let entry = linker
            .get_default(&mut store, "")?
            .typed::<(), ()>(&store)?;

        let join = std::thread::spawn(move || {
            let result = entry.call(&mut store, ());
            
            result
        });

        self.processes.push(join);
        
        Ok(())
    }
    */

    /// Start an aSi process from a module on the local disk.
    pub fn spawn_process_data(&mut self, wasi_data: &[u8]) -> anyhow::Result<()> {
        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;
        let mut wasi = WasiCtxBuilder::new().stdout(Box::new(OutputHandler{})).build();

        // Create the a-Si RPC root device.
        let sysreq_fd = wasi.push_file(Box::new(AsiSysreqDevice::new()), FileCaps::READ | FileCaps::WRITE)?;
        wasi.push_env("ASI_RPCROOT_FD", &sysreq_fd.to_string())?;

        let mut store = Store::new(&self.engine, wasi);

        let module = Module::from_binary(&self.engine, wasi_data)?;
        linker.module(&mut store, "", &module)?;

        let entry = linker
            .get_default(&mut store, "")?
            .typed::<(), ()>(&store)?;

        let join = std::thread::spawn(move || {
            let result = entry.call(&mut store, ());

            if let Err(err) = &result {
                log::warn!("Program crashed: {}", err)
            }
            
            result
        });

        self.processes.push(join);
        
        Ok(())
    }

    /// Wait for all processes to terminate. 
    pub fn wait(&mut self) {
        while let Some(join) = self.processes.pop() {
            let _ = join.join();
        }
    }
}

fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).filter_module("cranelift_codegen", LevelFilter::Warn).init();

    let mut control = match UdsControlServer::start("asi.sock", true) {
        Ok(server) => server,
        Err(err) => {
            log::error!("Failed to start control server: {}", err);
            std::process::exit(-1);
        },
    };

    let mut host = AsiBasicHost::new();

    loop {
        let request = match control.wait_request() {
            Ok(req) => req,
            Err(_) => {
                log::error!("Unexpected control server shutdown");
                break;
            },
        };

        match request.request() {
            ClientRequest::Version => {
                request.respond(Ok("1.0".as_bytes().to_vec()));
            }
            ClientRequest::Shutdown => {
                request.respond(Ok(vec![]));
                log::info!("Shutdown request, stopping host...");
                break;
            },
            ClientRequest::Run { binary } => {
                println!("Starting remote module...");
                if let Err(err) = host.spawn_process_data(binary) {
                    println!("Failed to start process: {}", err);
                    request.respond(Err("failed to start process".to_string()));
                } else {
                    request.respond(Ok(vec![]));
                }
            },
        }
    }

    control.shutdown();

    /*for module in std::env::args_os().skip(1) {
        println!("Starting module '{}'...", module.to_string_lossy());

        if let Err(err) = host.spawn_process_local(&module) {
            println!("Failed to start process: {}", err);
        }
    }*/

    host.wait();

    println!("All processes have terminated, host shut down.")
}
