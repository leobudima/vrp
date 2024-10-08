use super::*;
use crate::utils::DefaultRandom;

mod selection_sampling {
    use super::*;

    #[test]
    fn can_sample_from_large_range() {
        let random = Arc::new(DefaultRandom::default());
        let amount = 5;

        let numbers = SelectionSamplingIterator::new(0..100, amount, random).collect::<Vec<_>>();

        assert_eq!(numbers.len(), amount);
        numbers.windows(2).for_each(|item| match item {
            &[prev, next] => assert!(prev < next),
            _ => unreachable!(),
        });
        numbers.windows(2).any(|item| match item {
            &[prev, next] => prev + 1 < next,
            _ => false,
        });
    }

    #[test]
    fn can_sample_from_same_range() {
        let amount = 5;
        let random = Arc::new(DefaultRandom::default());

        let numbers = SelectionSamplingIterator::new(0..amount, amount, random).collect::<Vec<_>>();

        assert_eq!(numbers, vec![0, 1, 2, 3, 4])
    }

    #[test]
    fn can_sample_from_smaller_range() {
        let sample_size = 5;
        let random = Arc::new(DefaultRandom::default());

        let numbers = create_range_sampling_iter(0..3, sample_size, random.as_ref()).collect::<Vec<_>>();

        assert_eq!(numbers, vec![0, 1, 2])
    }
}

mod range_sampling {
    use super::*;
    use crate::prelude::RandomGen;

    struct DummyRandom {
        value: i32,
    }
    impl Random for DummyRandom {
        fn uniform_int(&self, min: i32, max: i32) -> i32 {
            assert!((min..=max).contains(&self.value));

            self.value
        }

        fn uniform_real(&self, _: Float, _: Float) -> Float {
            unimplemented!()
        }

        fn is_head_not_tails(&self) -> bool {
            unimplemented!()
        }

        fn is_hit(&self, _: Float) -> bool {
            unimplemented!()
        }

        fn weighted(&self, _: &[usize]) -> usize {
            unimplemented!()
        }

        fn get_rng(&self) -> RandomGen {
            unimplemented!()
        }
    }

    #[test]
    fn can_sample_from_large_range() {
        let sample_size = 5;
        let random = DummyRandom { value: 1 };

        let numbers = create_range_sampling_iter(0..100, sample_size, &random).collect::<Vec<_>>();

        assert_eq!(numbers, vec![5, 6, 7, 8, 9])
    }

    #[test]
    fn can_sample_from_same_range() {
        let sample_size = 5;
        let random = Arc::new(DefaultRandom::default());

        let numbers = create_range_sampling_iter(0..5, sample_size, random.as_ref()).collect::<Vec<_>>();

        assert_eq!(numbers, vec![0, 1, 2, 3, 4])
    }

    #[test]
    fn can_sample_from_smaller_range() {
        let sample_size = 5;
        let random = Arc::new(DefaultRandom::default());

        let numbers = create_range_sampling_iter(0..3, sample_size, random.as_ref()).collect::<Vec<_>>();

        assert_eq!(numbers, vec![0, 1, 2])
    }
}

mod sampling_search {
    use super::*;
    use crate::Environment;
    use std::cell::RefCell;

    #[derive(Clone, Debug, Default)]
    struct DataType {
        data: bool,
        idx: usize,
    }

    #[allow(clippy::type_complexity)]
    fn get_result_comparer(target: usize) -> Box<dyn Fn(&DataType, &DataType) -> bool> {
        Box::new(move |left, right| {
            match (left.data, right.data) {
                (true, false) => return true,
                (false, true) => return false,
                _ => {}
            }
            match (left.idx, right.idx) {
                (_, rhs) if rhs == target => false,
                (lhs, _) if lhs == target => true,
                (lhs, rhs) => (lhs as i32 - target as i32).abs() < (rhs as i32 - target as i32).abs(),
            }
        })
    }

    #[test]
    fn can_keep_evaluations_amount_low() {
        let total_size = 1000;
        let sample_size = 8;
        let target = 10;
        let random = Environment::default().random;

        let mut results = (0..100)
            .map(|_| {
                let mut counter = 0;
                let map_fn = |item: &DataType| {
                    counter += 1;
                    item.clone()
                };
                let compare_fn = get_result_comparer(target);
                let data = (0..total_size).map(|idx| DataType { data: idx % 2 == 0, idx }).collect::<Vec<_>>();

                let idx = data
                    .iter()
                    .sample_search(sample_size, random.clone(), map_fn, |item| item.idx, compare_fn)
                    .unwrap()
                    .idx;

                (idx, counter)
            })
            .collect::<Vec<_>>();

        results.sort_by(|(a, _), (b, _)| a.cmp(b));
        let median = results[results.len() / 2];
        assert!(median.0 < 250);
        assert!(results.iter().all(|(_, count)| *count < 100));
    }

    parameterized_test! {can_reproduce_issue_with_weak_sampling, (sequence, sample_size, expected_counter, expected_value), {
        can_reproduce_issue_with_weak_sampling_impl(sequence, sample_size, expected_counter, expected_value);
    }}

    can_reproduce_issue_with_weak_sampling! {
        case01_few_updates: (
            vec![
                76, 36, 93, 15, 21, 40, 97, 77, 35, 86, 61, 71, 7, 32, 29,
                66, 47, 96, 82, 34, 20, 23, 94, 11, 18, 89, 79, 47, 77, 30,
                48, 8, 45, 11, 21, 54, 15, 26, 23, 37, 58, 27, 31, 11, 60,
            ],
            4, 12, 96,
        ),
        case02_at_end: (
            vec![
                66, 47, 96, 82, 34, 20, 23, 94, 11, 18, 89, 79, 47, 77, 30,
                48, 8, 45, 11, 21, 54, 15, 26, 23, 37, 58, 27, 31, 11, 60,
                76, 36, 93, 15, 21, 40, 97, 77, 35, 86, 61, 71, 7, 32, 29,
            ],
            4, 7, 86,
        ),
        case03_wave: (
            vec![
                2, 5, 6, 10, 18, 24, 25, 29, 34, 35, 37, 38, 40, 43, 45, 53, 55, 60, 61, 63, 68,
                69, 71, 73, 77, 80, 81, 82, 84, 91, 96, 93, 90, 86, 80, 72, 71, 65, 62, 56, 55, 52,
            ],
            8, 13, 96,
        ),
    }

    fn can_reproduce_issue_with_weak_sampling_impl(
        sequence: Vec<i32>,
        sample_size: usize,
        expected_counter: usize,
        expected_value: i32,
    ) {
        let random = Arc::new(DefaultRandom::new_repeatable());
        let counter = RefCell::new(0);
        let value = sequence
            .into_iter()
            .enumerate()
            .sample_search(
                sample_size,
                random.clone(),
                |(_idx, i)| {
                    *counter.borrow_mut() += 1;
                    //println!("{} probe: {i} at {idx}", counter.borrow());
                    i
                },
                |(idx, _)| *idx,
                |a, b| *a > *b,
            )
            .unwrap();

        assert_eq!(value, expected_value);
        assert_eq!(*counter.borrow(), expected_counter);
    }
}
