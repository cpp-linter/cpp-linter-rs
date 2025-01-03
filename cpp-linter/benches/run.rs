use cpp_linter::run::run_main;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};

// This is a struct that tells Criterion.rs to use the tokio crate's multi-thread executor
use tokio::runtime::Runtime;
async fn run() -> Result<(), anyhow::Error> {
    let args = vec![
        "cpp-linter".to_string(),
        "--lines-changed-only".to_string(),
        "false".to_string(),
        "--database".to_string(),
        "benches/libgit2/build".to_string(),
        "--ignore".to_string(),
        "|!benches/libgit2/src/libgit2".to_string(),
        "--extensions".to_string(),
        "c".to_string(),
    ];
    run_main(args).await
}

fn bench1(c: &mut Criterion) {
    c.bench_function("scan libgit2", |b| {
        // Insert a call to `to_async` to convert the bencher to async mode.
        // The timing loops are the same as with the normal bencher.
        b.to_async(Runtime::new().unwrap()).iter(run);
    });
}

criterion_group!(benches, bench1);
criterion_main!(benches);
