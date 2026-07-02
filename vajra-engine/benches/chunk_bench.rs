

use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tempfile::tempdir;
use tokio::runtime::Runtime;
use vajra_engine::{
    allocator,
    writer::{start_disk_writer, DataFrame},
};

fn bench_allocate_file_space(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocator");
    let rt = Runtime::new().unwrap();

    let sizes = [(10 * 1024 * 1024, "10MB"), (100 * 1024 * 1024, "100MB")];

    for (size, name) in sizes.iter() {
        group.bench_with_input(
            BenchmarkId::new("allocate_file_space", name),
            size,
            |b, &s| {
                b.to_async(&rt).iter(|| async {
                    let dir = tempdir().unwrap();
                    let file_path = dir.path().join(format!("test_file_{}", s));
                    allocator::allocate_file_space(&file_path, s).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_chunk_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_merge");
    let rt = Runtime::new().unwrap();

    // 100 MB total size, 8 MB chunks
    let total_size: u64 = 100 * 1024 * 1024;
    let chunk_size: u64 = 8 * 1024 * 1024;

    group.throughput(Throughput::Bytes(total_size));

    group.bench_function("start_disk_writer_100MB", |b| {
        b.to_async(&rt).iter(|| async {
            let dir = tempdir().unwrap();
            let file_path = dir.path().join("merge_bench.bin");
            allocator::allocate_file_space(&file_path, total_size)
                .await
                .unwrap();

            let (tx, rx) = tokio::sync::mpsc::channel(256);

            let file_path_clone = file_path.clone();

            // Spawn writer
            let writer_fut =
                tokio::spawn(async move { start_disk_writer(&file_path_clone, rx).await.unwrap() });

            // Send frames
            let mut offset = 0;
            while offset < total_size {
                let frame_len = chunk_size.min(total_size - offset);
                // Pre-allocated zeros
                let payload = Bytes::from(vec![0u8; frame_len as usize]);

                tx.send(DataFrame {
                    absolute_offset: offset,
                    payload,
                })
                .await
                .unwrap();

                offset += frame_len;
            }
            drop(tx);

            writer_fut.await.unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, bench_allocate_file_space, bench_chunk_merge);
criterion_main!(benches);
