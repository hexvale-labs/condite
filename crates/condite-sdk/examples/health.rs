#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:8080".into());
    println!("{:?}", condite_sdk::Client::new(base)?.health().await?);
    Ok(())
}
