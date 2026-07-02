fn main() {
    let url = "magnet:?xt=urn:btih:c9e15763f722f23e98a29decdfae341b98d53056&dn=Cosmos+Laundromat&tr=udp%3A%2F%2Fexplodie.org%3A6969&tr=udp%3A%2F%2Ftracker.coppersurfer.tk%3A6969&tr=udp%3A%2F%2Ftracker.empire-js.us%3A1337&tr=udp%3A%2F%2Ftracker.leechers-paradise.org%3A6969&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337&tr=wss%3A%2F%2Ftracker.btorrent.xyz&tr=wss%3A%2F%2Ftracker.fastcast.nz&tr=wss%3A%2F%2Ftracker.openwebtorrent.com&ws=https%3A%2F%2Fwebtorrent.io%2Ftorrents%2F&xs=https%3A%2F%2Fwebtorrent.io%2Ftorrents%2Fcosmos-laundromat.torrent";

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let opts = librqbit::SessionOptions {
            disable_dht_persistence: true,
            listen_port_range: Some(6881..6890),
            ..Default::default()
        };

        let session = librqbit::Session::new_with_opts(
            "D:\\Project\\Project-Vajra\\vajra-daemon".into(),
            opts,
        )
        .await
        .unwrap();
        let add_torrent = librqbit::AddTorrent::from_url(url);

        println!("Adding torrent...");
        match session.add_torrent(add_torrent, None).await {
            Ok(_) => println!("Success!"),
            Err(e) => println!("Error: {:?}", e),
        }
        println!("Done!");
    });
}
