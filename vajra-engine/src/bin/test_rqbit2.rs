use std::time::Duration;

use librqbit::{AddTorrent, AddTorrentOptions, Session};

#[tokio::main]
async fn main() {
    let session = Session::new("test_dir_pause".into()).await.unwrap();
    let opts = AddTorrentOptions {
        overwrite: true,
        ..Default::default()
    };
    let handle = session
        .add_torrent(
            AddTorrent::from_url("magnet:?xt=urn:btih:c9e15763f722f23e98a29decdfae341b98d53056"),
            Some(opts),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    session.pause(&handle).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let opts2 = AddTorrentOptions {
        overwrite: true,
        paused: false,
        ..Default::default()
    };
    // Try to add it again to the SAME session!
    let res = session
        .add_torrent(
            AddTorrent::from_url("magnet:?xt=urn:btih:c9e15763f722f23e98a29decdfae341b98d53056"),
            Some(opts2),
        )
        .await;
    println!("Add torrent again result: {:?}", res.is_err());
    if let Err(e) = res {
        println!("Error: {}", e);
    }
}
