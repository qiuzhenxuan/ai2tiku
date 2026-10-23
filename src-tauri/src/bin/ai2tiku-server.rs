#[tokio::main]
async fn main() {
    if let Err(error) = ai2tiku_lib::hosted::run().await {
        eprintln!("Hosted server failed: {error}");
        std::process::exit(1);
    }
}
