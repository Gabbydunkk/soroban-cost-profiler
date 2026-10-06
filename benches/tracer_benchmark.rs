use criterion::{Criterion, criterion_group, criterion_main};
use soroban_cost_profiler::tracer::ExecutionTracer;

fn bench_record_step(c: &mut Criterion) {
    let mut tracer = ExecutionTracer::new().with_sample_rate(100);
    c.bench_function("record_step", |b| {
        b.iter(|| {
            let _ = tracer.record_step(
                std::hint::black_box(1),
                std::hint::black_box(10),
                std::hint::black_box(5),
            );
        })
    });
}

criterion_group!(benches, bench_record_step);
criterion_main!(benches);
