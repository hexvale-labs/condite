use condite_core::start_app;

#[tokio::main]
async fn main() {
    condite_core::logging::init().expect("logging initialization failed");

    start_app().await;
}
