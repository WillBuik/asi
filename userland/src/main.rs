fn main() {
    println!("Hello WASI");

    _ = libasi::log::init();
    log::info!("Hello a-Si log");

    libasi::hello("sysreq");

    log::info!("{:?}", libasi::net::lookup("miats.com:80"));
    
    log::info!("done");

    let start = std::time::Instant::now();
    for _ in 0..225000 {
        libasi::poke();
    }
    let bench = std::time::Instant::now() - start;
    log::info!("Duration: {}", bench.as_secs_f32())
}
