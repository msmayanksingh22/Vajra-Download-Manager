use librqbit::{AddTorrent, Session};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let session = Session::new("D:/Project/Project-Vajra/test_downloads".into()).await?;

    // Ubuntu 24.04 desktop magnet
    let magnet = "magnet:?xt=urn:btih:3f23a4130090280eb4c2dbcdb72b62d854eb0c16&dn=ubuntu-24.04.1-desktop-amd64.iso";

    let handle = session
        .add_torrent(AddTorrent::from_url(magnet), None)
        .await?
        .into_handle()
        .unwrap();

    let stats = handle.stats();
    println!("{:?}", stats);

    Ok(())
}
