// SPDX-License-Identifier: MIT
//! Development entry point; no game operations are implemented yet.

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None | Some("--help" | "-h") => {
            println!(
                "Cobble Launcher — development scaffold\n\nUsage: cobble [--help | --version]\nGame installation and launch are not implemented."
            );
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            let info = cobble_core::app_info();
            println!("{} {} ({})", info.name, info.version, info.stage);
            ExitCode::SUCCESS
        }
        Some(_) => {
            eprintln!("Unsupported command: this scaffold cannot install or launch games.");
            ExitCode::from(2)
        }
    }
}
