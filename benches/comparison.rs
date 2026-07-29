use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use fracindex::Fracindex;
use fractional_index::FractionalIndex;
use std::{hint::black_box, time::Duration};

const KEY_SIZES: [(&str, usize); 3] = [("short", 1), ("16_bytes", 16), ("64_bytes", 64)];
const WORKLOAD_SIZES: [u64; 3] = [100, 1_000, 10_000];

trait BenchIndex: Ord + Sized {
    fn initial() -> Self;
    fn new_before(other: &Self) -> Self;
    fn new_after(other: &Self) -> Self;
    fn new_between(left: &Self, right: &Self) -> Option<Self>;
    fn byte_len(&self) -> usize;
}

impl BenchIndex for Fracindex {
    fn initial() -> Self {
        Self::default()
    }

    fn new_before(other: &Self) -> Self {
        Self::new_before(other, fracindex::FracindexPolicy::Sequential)
    }

    fn new_after(other: &Self) -> Self {
        Self::new_after(other, fracindex::FracindexPolicy::Sequential)
    }

    fn new_between(left: &Self, right: &Self) -> Option<Self> {
        Self::new_between(left, right)
    }

    fn byte_len(&self) -> usize {
        self.to_bytes().len()
    }
}

impl BenchIndex for FractionalIndex {
    fn initial() -> Self {
        Self::default()
    }

    fn new_before(other: &Self) -> Self {
        Self::new_before(other)
    }

    fn new_after(other: &Self) -> Self {
        Self::new_after(other)
    }

    fn new_between(left: &Self, right: &Self) -> Option<Self> {
        Self::new_between(left, right)
    }

    fn byte_len(&self) -> usize {
        self.as_bytes().len()
    }
}

fn validate_adapter<T: BenchIndex>() {
    let index = T::initial();
    let before = T::new_before(&index);
    let after = T::new_after(&index);
    let between = T::new_between(&index, &after).expect("ordered bounds must have a midpoint");

    assert!(before < index);
    assert!(index < between);
    assert!(between < after);
    assert!(T::new_between(&after, &index).is_none());
}

fn grow_before<T: BenchIndex>(target_len: usize) -> T {
    let mut index = T::initial();
    while index.byte_len() < target_len {
        index = T::new_before(&index);
    }
    index
}

fn grow_after<T: BenchIndex>(target_len: usize) -> T {
    let mut index = T::initial();
    while index.byte_len() < target_len {
        index = T::new_after(&index);
    }
    index
}

fn grow_between<T: BenchIndex>(target_len: usize) -> (T, T) {
    let left = T::initial();
    let mut right = T::new_after(&left);

    while left.byte_len().max(right.byte_len()) < target_len {
        right = T::new_between(&left, &right).expect("ordered bounds must have a midpoint");
    }

    assert!(left < right);
    (left, right)
}

fn append_workload<T: BenchIndex>(count: u64) -> T {
    let mut index = T::initial();
    for _ in 0..count {
        index = T::new_after(&index);
    }
    index
}

fn prepend_workload<T: BenchIndex>(count: u64) -> T {
    let mut index = T::initial();
    for _ in 0..count {
        index = T::new_before(&index);
    }
    index
}

fn dense_between_workload<T: BenchIndex>(count: u64) -> T {
    let left = T::initial();
    let mut right = T::new_after(&left);

    for _ in 0..count {
        right = T::new_between(&left, &right).expect("ordered bounds must have a midpoint");
    }

    right
}

fn benchmark_default(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/default");

    group.bench_function("fracindex", |b| b.iter(|| black_box(Fracindex::initial())));
    group.bench_function("fractional_index", |b| {
        b.iter(|| black_box(FractionalIndex::initial()))
    });

    group.finish();
}

fn benchmark_before(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/new_before");

    for (label, target_len) in KEY_SIZES {
        let fracindex = grow_before::<Fracindex>(target_len);
        let fractional_index = grow_before::<FractionalIndex>(target_len);

        group.bench_with_input(
            BenchmarkId::new("fracindex", label),
            &fracindex,
            |b, index| {
                b.iter(|| {
                    black_box(Fracindex::new_before(
                        black_box(index),
                        fracindex::FracindexPolicy::Sequential,
                    ))
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fractional_index", label),
            &fractional_index,
            |b, index| b.iter(|| black_box(FractionalIndex::new_before(black_box(index)))),
        );
    }

    group.finish();
}

fn benchmark_after(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/new_after");

    for (label, target_len) in KEY_SIZES {
        let fracindex = grow_after::<Fracindex>(target_len);
        let fractional_index = grow_after::<FractionalIndex>(target_len);

        group.bench_with_input(
            BenchmarkId::new("fracindex", label),
            &fracindex,
            |b, index| {
                b.iter(|| {
                    black_box(Fracindex::new_after(
                        black_box(index),
                        fracindex::FracindexPolicy::Sequential,
                    ))
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fractional_index", label),
            &fractional_index,
            |b, index| b.iter(|| black_box(FractionalIndex::new_after(black_box(index)))),
        );
    }

    group.finish();
}

fn benchmark_between(c: &mut Criterion) {
    let mut group = c.benchmark_group("operations/new_between");

    for (label, target_len) in KEY_SIZES {
        let fracindex = grow_between::<Fracindex>(target_len);
        let fractional_index = grow_between::<FractionalIndex>(target_len);

        group.bench_with_input(
            BenchmarkId::new("fracindex", label),
            &fracindex,
            |b, (left, right)| {
                b.iter(|| {
                    black_box(
                        Fracindex::new_between(black_box(left), black_box(right))
                            .expect("fixture bounds must have a midpoint"),
                    )
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fractional_index", label),
            &fractional_index,
            |b, (left, right)| {
                b.iter(|| {
                    black_box(
                        FractionalIndex::new_between(black_box(left), black_box(right))
                            .expect("fixture bounds must have a midpoint"),
                    )
                })
            },
        );
    }

    group.finish();
}

fn benchmark_workload<T: BenchIndex>(count: u64, workload: fn(u64) -> T) -> T {
    workload(count)
}

fn benchmark_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("workloads/append");

    for count in WORKLOAD_SIZES {
        group.throughput(Throughput::Elements(count));
        group.bench_with_input(BenchmarkId::new("fracindex", count), &count, |b, count| {
            b.iter(|| {
                black_box(benchmark_workload::<Fracindex>(
                    black_box(*count),
                    append_workload::<Fracindex>,
                ))
            })
        });
        group.bench_with_input(
            BenchmarkId::new("fractional_index", count),
            &count,
            |b, count| {
                b.iter(|| {
                    black_box(benchmark_workload::<FractionalIndex>(
                        black_box(*count),
                        append_workload::<FractionalIndex>,
                    ))
                })
            },
        );
    }

    group.finish();
}

fn benchmark_prepend(c: &mut Criterion) {
    let mut group = c.benchmark_group("workloads/prepend");

    for count in WORKLOAD_SIZES {
        group.throughput(Throughput::Elements(count));
        group.bench_with_input(BenchmarkId::new("fracindex", count), &count, |b, count| {
            b.iter(|| {
                black_box(benchmark_workload::<Fracindex>(
                    black_box(*count),
                    prepend_workload::<Fracindex>,
                ))
            })
        });
        group.bench_with_input(
            BenchmarkId::new("fractional_index", count),
            &count,
            |b, count| {
                b.iter(|| {
                    black_box(benchmark_workload::<FractionalIndex>(
                        black_box(*count),
                        prepend_workload::<FractionalIndex>,
                    ))
                })
            },
        );
    }

    group.finish();
}

fn benchmark_dense_between(c: &mut Criterion) {
    let mut group = c.benchmark_group("workloads/dense_between");

    for count in WORKLOAD_SIZES {
        group.throughput(Throughput::Elements(count));
        group.bench_with_input(BenchmarkId::new("fracindex", count), &count, |b, count| {
            b.iter(|| {
                black_box(benchmark_workload::<Fracindex>(
                    black_box(*count),
                    dense_between_workload::<Fracindex>,
                ))
            })
        });
        group.bench_with_input(
            BenchmarkId::new("fractional_index", count),
            &count,
            |b, count| {
                b.iter(|| {
                    black_box(benchmark_workload::<FractionalIndex>(
                        black_box(*count),
                        dense_between_workload::<FractionalIndex>,
                    ))
                })
            },
        );
    }

    group.finish();
}

fn benchmarks(c: &mut Criterion) {
    validate_adapter::<Fracindex>();
    validate_adapter::<FractionalIndex>();

    benchmark_default(c);
    benchmark_before(c);
    benchmark_after(c);
    benchmark_between(c);
    benchmark_append(c);
    benchmark_prepend(c);
    benchmark_dense_between(c);
}

criterion_group! {
    name = comparison;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .sample_size(50);
    targets = benchmarks
}
criterion_main!(comparison);
