fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:8080".into());
    let client = condite_sdk::Client::new(base)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    println!("{:?}", runtime.block_on(client.health())?);
    Ok(())
}
