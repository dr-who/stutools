mod allocation;
#[global_allocator]
static ALLOCATOR: allocation::Audited = allocation::Audited;
mod cli;
mod engine;
mod io;
mod pattern;
mod positions;
fn main() {
    let result = cli::parse(std::env::args().skip(1).collect()).and_then(|config| {
        if let Some(config) = config {
            engine::run(config)
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
