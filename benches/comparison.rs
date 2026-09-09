use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use fracindex::{Fracindex, SpacePolicy};
use fractional_index::FractionalIndex;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{hint::black_box, time::Duration};

const KEY_SIZES: [(&str, usize); 3] = [("short", 1), ("16_bytes", 16), ("64_bytes", 64)];
const WORKLOAD_SIZES: [u64; 10] = [
    10_000, 20_000, 30_000, 40_000, 50_000, 60_000, 70_000, 80_000, 90_000, 100_000,
];
const RANDOM_WORKLOAD_SEED: u64 = 0xF12A_C710_1D3E_5EED;

trait BenchIndex: Ord + Sized {
    fn initial() -> Self;
    fn before_index(other: &Self) -> Self;
    fn after_index(other: &Self) -> Self;
    fn between_indexes(left: &Self, right: &Self) -> Self;
    fn byte_len(&self) -> usize;
    fn before_index_prefer_random(other: &Self) -> Self {
        Self::before_index(other)
    }
    fn after_index_prefer_random(other: &Self) -> Self {
        Self::after_index(other)
    }
}

impl BenchIndex for Fracindex {
    fn initial() -> Self {
        Self::default()
    }

    fn before_index(other: &Self) -> Self {
        Self::builder().before(other).build().unwrap()
    }

    fn after_index(other: &Self) -> Self {
        Self::builder().after(other).build().unwrap()
    }

    fn between_indexes(left: &Self, right: &Self) -> Self {
        Self::builder().between(left, right).build().unwrap()
    }

    fn byte_len(&self) -> usize {
        self.to_bytes().len()
    }

    fn before_index_prefer_random(other: &Self) -> Self {
        Self::builder()
            .before(other)
            .space_policy(SpacePolicy::Random)
            .build()
            .unwrap()
    }

    fn after_index_prefer_random(other: &Self) -> Self {
        Self::builder()
            .after(other)
            .space_policy(SpacePolicy::Random)
            .build()
            .unwrap()
    }
}

impl BenchIndex for FractionalIndex {
    fn initial() -> Self {
        Self::default()
    }

    fn before_index(other: &Self) -> Self {
        Self::new_before(other)
    }

    fn after_index(other: &Self) -> Self {
        Self::new_after(other)
    }

    fn between_indexes(left: &Self, right: &Self) -> Self {
        Self::new_between(left, right).unwrap()
    }

    fn byte_len(&self) -> usize {
        self.as_bytes().len()
    }
}

fn validate_adapter<T: BenchIndex>() {
    let index = T::initial();
    let before = T::before_index(&index);
    let after = T::after_index(&index);
    let between = T::between_indexes(&index, &after);

    assert!(before < index);
    assert!(index < between);
    assert!(between < after);
}

fn grow_before<T: BenchIndex>(target_len: usize) -> T {
    let mut index = T::initial();
    while index.byte_len() < target_len {
        index = T::before_index(&index);
    }
    index
}

fn grow_after<T: BenchIndex>(target_len: usize) -> T {
    let mut index = T::initial();
    while index.byte_len() < target_len {
        index = T::after_index(&index);
    }
    index
}

fn grow_fracindex_before(target_len: usize) -> Fracindex {
    let mut index = Fracindex::default();
    while index.to_bytes().len() < target_len {
        index = Fracindex::builder()
            .before(&index)
            .space_policy(SpacePolicy::Random)
            .build()
            .unwrap();
    }
    index
}

fn grow_fracindex_after(target_len: usize) -> Fracindex {
    let mut index = Fracindex::default();
    while index.to_bytes().len() < target_len {
        index = Fracindex::builder()
            .after(&index)
            .space_policy(SpacePolicy::Random)
            .build()
            .unwrap();
    }
    index
}

fn grow_between<T: BenchIndex>(target_len: usize) -> (T, T) {
    let left = T::initial();
    let mut right = T::after_index(&left);

    while left.byte_len().max(right.byte_len()) < target_len {
        right = T::between_indexes(&left, &right);
    }

    assert!(left < right);
    (left, right)
}

fn append_workload<T: BenchIndex>(count: u64) -> T {
    let mut index = T::initial();
    for _ in 0..count {
        index = T::after_index(&index);
    }
    index
}

fn prepend_workload<T: BenchIndex>(count: u64) -> T {
    let mut index = T::initial();
    for _ in 0..count {
        index = T::before_index(&index);
    }
    index
}

fn dense_between_workload<T: BenchIndex>(count: u64) -> T {
    let left = T::initial();
    let mut right = T::after_index(&left);

    for _ in 0..count {
        right = T::between_indexes(&left, &right);
    }

    right
}

type Gap = (Option<usize>, Option<usize>);

struct RandomInsertWorkload<T> {
    indexes: Vec<T>,
    gaps: Vec<Gap>,
}

fn random_gap_choices(count: u64) -> Vec<usize> {
    let count = usize::try_from(count).expect("workload size must fit in usize");
    let mut rng = StdRng::seed_from_u64(RANDOM_WORKLOAD_SEED);

    (0..count)
        .map(|inserted| rng.random_range(0..inserted + 2))
        .collect()
}

fn random_insert_workload<T: BenchIndex, const VALIDATE: bool>(
    choices: &[usize],
) -> RandomInsertWorkload<T> {
    let mut indexes = Vec::with_capacity(choices.len() + 1);
    indexes.push(T::initial());

    let mut gaps = Vec::with_capacity(choices.len() + 2);
    gaps.push((None, Some(0)));
    gaps.push((Some(0), None));

    for (iteration, &choice) in choices.iter().enumerate() {
        let (left, right) = gaps[choice];
        let index = match (left, right) {
            (None, Some(right)) => T::before_index_prefer_random(&indexes[right]),
            (Some(left), None) => T::after_index_prefer_random(&indexes[left]),
            (Some(left), Some(right)) => T::between_indexes(&indexes[left], &indexes[right]),
            (None, None) => unreachable!("a gap must have at least one bound"),
        };

        if VALIDATE {
            if let Some(left) = left {
                assert!(indexes[left] < index, "iteration {iteration}, left bound");
            }
            if let Some(right) = right {
                assert!(index < indexes[right], "iteration {iteration}, right bound");
            }
        }

        let inserted = indexes.len();
        indexes.push(index);
        gaps[choice] = (left, Some(inserted));
        gaps.push((Some(inserted), right));
    }

    RandomInsertWorkload { indexes, gaps }
}

fn validate_random_insert_workload<T: BenchIndex>() {
    let random_choices = random_gap_choices(1_000);

    for choices in [&[0][..], &[1][..], random_choices.as_slice()] {
        let workload = random_insert_workload::<T, true>(choices);
        assert_eq!(workload.indexes.len(), choices.len() + 1);
        assert_eq!(workload.gaps.len(), choices.len() + 2);

        for &(left, right) in &workload.gaps {
            if let (Some(left), Some(right)) = (left, right) {
                assert!(workload.indexes[left] < workload.indexes[right]);
            }
        }
    }
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
    let mut group = c.benchmark_group("operations/builder_before");

    for (label, target_len) in KEY_SIZES {
        let fracindex = grow_fracindex_before(target_len);
        let fractional_index = grow_before::<FractionalIndex>(target_len);

        group.bench_with_input(
            BenchmarkId::new("fracindex", label),
            &fracindex,
            |b, index| {
                b.iter(|| {
                    black_box(
                        Fracindex::builder()
                            .before(black_box(index))
                            .build()
                            .unwrap(),
                    )
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
    let mut group = c.benchmark_group("operations/builder_after");

    for (label, target_len) in KEY_SIZES {
        let fracindex = grow_fracindex_after(target_len);
        let fractional_index = grow_after::<FractionalIndex>(target_len);

        group.bench_with_input(
            BenchmarkId::new("fracindex", label),
            &fracindex,
            |b, index| {
                b.iter(|| {
                    black_box(
                        Fracindex::builder()
                            .after(black_box(index))
                            .build()
                            .unwrap(),
                    )
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
    let mut group = c.benchmark_group("operations/builder_between");

    for (label, target_len) in KEY_SIZES {
        let fracindex = grow_between::<Fracindex>(target_len);
        let fractional_index = grow_between::<FractionalIndex>(target_len);

        group.bench_with_input(
            BenchmarkId::new("fracindex", label),
            &fracindex,
            |b, (left, right)| {
                b.iter(|| {
                    black_box(
                        Fracindex::builder()
                            .between(black_box(left), black_box(right))
                            .build()
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

fn benchmark_random_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("workloads/random_insert");

    for count in WORKLOAD_SIZES {
        let choices = random_gap_choices(count);
        group.throughput(Throughput::Elements(count));
        group.bench_with_input(
            BenchmarkId::new("fracindex", count),
            &choices,
            |b, choices| {
                b.iter(|| {
                    black_box(random_insert_workload::<Fracindex, false>(black_box(
                        choices,
                    )))
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fractional_index", count),
            &choices,
            |b, choices| {
                b.iter(|| {
                    black_box(random_insert_workload::<FractionalIndex, false>(black_box(
                        choices,
                    )))
                })
            },
        );
    }

    group.finish();
}

fn benchmarks(c: &mut Criterion) {
    validate_adapter::<Fracindex>();
    validate_adapter::<FractionalIndex>();
    validate_random_insert_workload::<Fracindex>();
    validate_random_insert_workload::<FractionalIndex>();

    benchmark_default(c);
    benchmark_before(c);
    benchmark_after(c);
    benchmark_between(c);
    benchmark_append(c);
    benchmark_prepend(c);
    benchmark_dense_between(c);
    benchmark_random_insert(c);
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
