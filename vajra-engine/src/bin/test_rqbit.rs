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
    drop(handle);
    drop(session);

    let session = Session::new("test_dir_pause".into()).await.unwrap();
    let opts2 = AddTorrentOptions {
        overwrite: true,
        paused: false,
        ..Default::default()
    };
    let handle = session
        .add_torrent(
            AddTorrent::from_url("magnet:?xt=urn:btih:c9e15763f722f23e98a29decdfae341b98d53056"),
            Some(opts2),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("State initially: {:?}", handle.stats().state);
}
