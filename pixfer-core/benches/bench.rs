use criterion::{criterion_group, criterion_main, Criterion};
use pixfer_core::protocol::{Frame, FrameKind, Protocol};
use pixfer_core::rs::{ReedSolomon, RS_K};

fn bench_rs(c: &mut Criterion) {
    let mut group = c.benchmark_group("reed-solomon");
    let msg = vec![0x42u8; RS_K];
    let rs = ReedSolomon::new();
    group.bench_function("encode 223B", |bench| {
        bench.iter(|| rs.encode(&msg));
    });
    let coded = rs.encode(&msg);
    group.bench_function("decode clean 255B", |bench| {
        bench.iter(|| rs.decode(&coded).unwrap());
    });
    let mut corrupted = coded.clone();
    for i in 0..16 {
        corrupted[i] ^= 0xFF;
    }
    group.bench_function("decode with 16 errors", |bench| {
        bench.iter(|| rs.decode(&corrupted).unwrap());
    });
    group.finish();
}

fn bench_protocol(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol");
    let frame = Frame::new(FrameKind::Chunk, 7, 1, 9, vec![0x42u8; 223]);
    group.bench_function("render 270x480 grid", |bench| {
        bench.iter(|| Protocol::render(&frame));
    });
    let blocks = Protocol::render(&frame);
    group.bench_function("decode 270x480 grid", |bench| {
        bench.iter(|| Protocol::decode(&blocks).unwrap());
    });
    group.finish();
}

criterion_group! {
    name: "pixfer",
    functions: [bench_rs, bench_protocol],
    targets: []
}
criterion_main!(pixfer);
