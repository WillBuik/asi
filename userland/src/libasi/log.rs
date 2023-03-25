use libasi_interop::diagnostics::LogRpcRequest;
use log::{SetLoggerError, LevelFilter, Metadata, Record};

use crate::libasi::rpc::rpc_call;

struct AsiLogger;

impl log::Log for AsiLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let log_rpc_req = LogRpcRequest {
                target: record.target().to_string(),
                level: record.level() as u32,
                body: format!("{}", record.args()),
                module_path: record.module_path().map(String::from),
                file: record.file().map(String::from),
                line: record.line(),
            };
            rpc_call(&log_rpc_req);
        }
    }

    fn flush(&self) {}
}

static LOGGER: AsiLogger = AsiLogger;

pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
}
