use condite_core::start_app;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    start_app().await;
}