#[tokio::main]
async fn main() {
    my_music_player::start_server(true).await;
}
