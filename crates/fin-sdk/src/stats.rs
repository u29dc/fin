pub(crate) fn median_i64(values: &[i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted.get(mid).copied()
    } else {
        let left = sorted.get(mid.saturating_sub(1)).copied().unwrap_or(0);
        let right = sorted.get(mid).copied().unwrap_or(0);
        Some((left + right) / 2)
    }
}

pub(crate) fn mean_i64(values: &[i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let sum = values.iter().sum::<i64>();
    let count = i64::try_from(values.len()).ok()?;
    Some(sum / count)
}

pub(crate) fn round_ratio(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::{mean_i64, median_i64, round_ratio};

    #[test]
    fn median_handles_even_and_odd_sets() {
        assert_eq!(median_i64(&[]), None);
        assert_eq!(median_i64(&[5]), Some(5));
        assert_eq!(median_i64(&[9, 1, 5]), Some(5));
        assert_eq!(median_i64(&[2, 6, 10, 14]), Some(8));
    }

    #[test]
    fn mean_returns_none_for_empty_input() {
        assert_eq!(mean_i64(&[]), None);
        assert_eq!(mean_i64(&[10, 20, 30]), Some(20));
    }

    #[test]
    fn ratio_rounding_matches_archive_precision() {
        assert!((round_ratio(1.236) - 1.24).abs() < f64::EPSILON);
        assert!((round_ratio(0.804) - 0.8).abs() < f64::EPSILON);
    }
}
