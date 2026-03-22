#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    cortex_api::run().await
}
