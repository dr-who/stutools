mod allocation;
#[global_allocator]
static ALLOCATOR: allocation::Audited = allocation::Audited;
mod aligned;
mod cctv;
mod cctv_plots;
mod cctv_test;
mod checked_close;
mod cli;
mod engine;
mod fsfiller;
mod info;
mod io;
mod iscsi;
mod memory;
mod nvme;
mod paths;
mod pattern;
mod positions;
mod progress;
mod rate;
mod signals;
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(pos) = args.iter().position(|a| a == "--cctv-test") {
        // The standard test: `--cctv-test` (short, the default), `--cctv-test range` or `--cctv-test long`.
        args.remove(pos);
        let mode = match args.get(pos) {
            Some(m) if m == "short" || m == "long" || m == "range" => args.remove(pos),
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
            if let Some(settings) = config.fsfiller {
                fsfiller::run(settings)
            } else if config.cameras.is_some() {
                cctv::run(config)
            } else if !config.nvmeof.is_empty() && config.jobs.is_empty() {
                if !config.iscsi.is_empty() {
                    Err("paths of both kinds need a -c workload to run over them".into())
                } else if config.nvmeof.iter().all(|t| !t.contains('/')) {
                    nvme::discover(&config.nvmeof)
                } else if config.nvmeof.iter().any(|t| !t.contains('/')) {
                    Err(
                        "--nvmeof takes HOST alone to list subsystems, or HOST/NQN to identify one"
                            .into(),
                    )
                } else {
                    nvme::report(&config.nvmeof)
                }
            } else if !config.iscsi.is_empty() && config.jobs.is_empty() {
                if config.iscsi.iter().all(|t| !t.contains('/')) {
                    let initiator = config.iscsi_initiator.clone().unwrap_or_default();
                    iscsi::report::discover(&config.iscsi, &initiator)
                } else if config.iscsi.iter().any(|t| !t.contains('/')) {
                    Err(
                        "--iscsi takes HOST alone to list targets, or HOST/IQN to report one"
                            .into(),
                    )
                } else if config.compliance {
                    iscsi::compliance::run(&config)
                } else {
                    iscsi::report::run(&config)
                }
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
