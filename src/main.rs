use std::process;

fn main() {
    let cli = worklouderctl::cli::parse_from(std::env::args_os());
    if let Err(error) = worklouderctl::run(cli, std::io::stdout()) {
        eprintln!("error: {error:#}");
        process::exit(1);
    }
}
