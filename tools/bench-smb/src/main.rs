//! bench-smb: measures how a photo folder on a NAS/SMB share behaves under the
//! reads PhotoSee will issue, so the server's IO concurrency limit rests on data.
//! Read-only: files are only ever opened for reading.

mod args;
mod bench;
mod phases;
mod report;
mod scan;
mod stats;
mod sweep;
#[cfg(test)]
mod testutil;
mod timefmt;

use std::process::ExitCode;

fn main() -> ExitCode {
    match args::parse(std::env::args().skip(1)) {
        Ok(args::Parsed::Help) => {
            print!("{}", args::USAGE);
            ExitCode::SUCCESS
        }
        Ok(args::Parsed::Run { dir, runs }) => {
            let (text, listed) = bench::run(&dir, runs);
            print!("{text}");
            if listed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("bench-smb: {e}\n\n{}", args::USAGE);
            ExitCode::from(2)
        }
    }
}
