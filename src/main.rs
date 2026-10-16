use std::env;
use std::process;

use stock_monitor::cli::{Command, Config, HELP};

fn main() {
    let config = match Config::build(env::args()) {
        Ok(Command::Monitor(config)) => config,
        Ok(Command::ShowHelp) => {
            print!("{HELP}");
            return;
        }
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(2);
        }
    };

    if let Err(err) = stock_monitor::run(&config) {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
