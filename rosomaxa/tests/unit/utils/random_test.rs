use super::*;

#[test]
fn can_return_weights() {
    let random = DefaultRandom::default();
    let weights = &[100, 50, 20];
    let experiments = 10000_usize;
    let total_sum = weights.iter().sum::<usize>();
    let mut counter = [0_usize; 3];

    (0..experiments).for_each(|_| {
        let idx = random.weighted(weights);
        *counter.get_mut(idx).unwrap() += 1;
    });

    weights.iter().enumerate().for_each(|(idx, weight)| {
        let actual_ratio = counter[idx] as Float / experiments as Float;
        let expected_ratio = *weight as Float / total_sum as Float;

        assert!((actual_ratio - expected_ratio).abs() < 0.05);
    });
}
