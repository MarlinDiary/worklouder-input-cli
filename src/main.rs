use std::process;

fn main() {
    let cli = worklouderctl::cli::parse_from(std::env::args_os());
    let json = cli.json;
    if let Err(error) = worklouderctl::run(cli, std::io::stdout()) {
        let report = worklouderctl::exit_status::report(&error);
        if json {
            if serde_json::to_writer_pretty(std::io::stderr(), &report).is_ok() {
                eprintln!();
            } else {
                eprintln!("error[{}]: {}", report.code.as_str(), report.message);
            }
        } else {
            eprintln!("error[{}]: {}", report.code.as_str(), report.message);
        }
        process::exit(report.exit_status);
    }
}
