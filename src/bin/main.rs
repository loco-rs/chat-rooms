use loco_rs::cli;
use chat_rooms::app::App;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    cli::main::<App>().await
}
