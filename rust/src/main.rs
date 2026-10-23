mod allocation;
#[global_allocator]
static ALLOCATOR: allocation::Audited = allocation::Audited;
mod cctv;
mod cctv_plots;
mod cctv_test;
mod cli;
mod engine;
mod info;
mod io;
mod memory;
mod pattern;
mod positions;
mod progress;
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(pos) = args.iter().position(|a| a == "--cctv-test") {
        // The standard test: `--cctv-test` (short, the default) or `--cctv-test long`.
        args.remove(pos);
        let mode = match args.get(pos) {
            Some(m) if m == "short" || m == "long" => args.remove(pos),
            _ => "short".to_string(),
        };
        if let Err(error) = cctv_test::run(&mode, args) {
            eprintln!("*error* {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.len() == 2 && args[0] == "--info" {
        // Describes a block device (identity, queue and transfer limits, iSCSI session); no I/O.
        if let Err(error) = info::run(&args[1]) {
            eprintln!("*error* {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.len() == 2 && args[0] == "--cctv-plots" {
        // Rebuilds the CCTV plot files from the cctv-test-*.lat files already in DIR.
        if let Err(error) = cctv::replot(std::path::Path::new(&args[1])) {
            eprintln!("*error* {error}");
            std::process::exit(1);
        }
        return;
    }
    let result = cli::parse(args).and_then(|config| {
        if let Some(config) = config {
            if config.cameras.is_some() {
                cctv::run(config)
            } else {
                engine::run(config).map(|_| ())
            }
        } else {
            Ok(())
        }
    });
    if let Err(error) = result {
        eprintln!("*error* {error}");
        std::process::exit(1);
    }
}
type Result<T> = std::result::Result<T, String>;
