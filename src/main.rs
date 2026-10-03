use anyhow::{Result, bail};

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        None => cornercase::client::run()?,
        Some("server") => cornercase::server::run()?,
        Some("--version") => println!("cornercase {}", cornercase::update::CURRENT),
        Some("kill-server") => {
            if !cornercase::client::kill_server()? {
                eprintln!("no cornercase server is running");
            }
        }
        Some(other) => {
            bail!("unknown command `{other}`; use `cornercase`, `cornercase kill-server` or `cornercase --version`")
        }
    }
    Ok(())
}
