use std::time::Duration;

use librqbit::{AddTorrent, AddTorrentOptions, Session};

#[tokio::main]
async fn main() {
    let session = Session::new("test_dir_ubuntu".into()).await.unwrap();
    let opts = AddTorrentOptions {
        overwrite: true,
        ..Default::default()
    };
    // Ubuntu 24.04 desktop amd64 torrent
    let handle = session.add_torrent(AddTorrent::from_url("magnet:?xt=urn:btih:e3811b9539cacff680e418124272177c4740156c&dn=ubuntu-24.04-desktop-amd64.iso&tr=https%3A%2F%2Ftorrent.ubuntu.com%2Fannounce&tr=https%3A%2F%2Fipv6.torrent.ubuntu.com%2Fannounce"), Some(opts)).await.unwrap().into_handle().unwrap();

    // Wait until it has a name
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        if let Some(name) = handle.name() {
            println!("Got name: {}", name);
            break;
        }
    }
}
