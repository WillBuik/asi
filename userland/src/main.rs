use std::time::Instant;

pub mod libasi;

fn main() {
    println!("Hello WASI");

    _ = libasi::log::init();
    log::info!("Hello a-Si log");

    libasi::hello("sysreq");

    log::info!("{:?}", libasi::net::lookup("miats.com:80"));

    let start = Instant::now();
    for _ in 0..225000 {
        libasi::poke();
    }
    let bench = Instant::now() - start;
    log::info!("Duration: {}", bench.as_secs_f32())
}
