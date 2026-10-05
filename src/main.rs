use std::process::ExitCode;

use anyhow::{Result, bail};

fn main() -> Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => cornercase::client::run()?,
        Some("server") => cornercase::server::run()?,
        Some("--version") => println!("cornercase {}", cornercase::update::CURRENT),
        Some("kill-server") => {
            if !cornercase::client::kill_server()? {
                eprintln!("no cornercase server is running");
            }
        }
        Some("update") => return update(&args[1..]),
        Some(other) => bail!(
            "unknown command `{other}`; use `cornercase`, `cornercase update`, `cornercase kill-server` \
             or `cornercase --version`"
        ),
    }
    Ok(ExitCode::SUCCESS)
}

fn update(options: &[String]) -> Result<ExitCode> {
    let (mut check_only, mut yes) = (false, false);
    for option in options {
        match option.as_str() {
            "--check" => check_only = true,
            "-y" | "--yes" => yes = true,
            other => bail!("unknown option `{other}`; use `cornercase update [--check] [--yes]`"),
        }
    }
    Ok(if cornercase::client::update(check_only, yes)? { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}
