use anyhow::Result;
use fin_sdk::sdk_banner;

fn main() -> Result<()> {
    println!("fin-tui bootstrap - {}", sdk_banner());
    Ok(())
}
