/// Adjusts the given vector to match the required length.
pub fn adjust_vector_length<T: Default + Clone>(vec: &mut Vec<T>, required_length: usize) {
    match vec.len().cmp(&required_length) {
        std::cmp::Ordering::Less => {
            let elements_to_append = required_length - vec.len();
            vec.extend(vec![T::default(); elements_to_append]);
        }
        std::cmp::Ordering::Greater => {
            vec.truncate(required_length);
        }
        _ => {}
    }
}

/// Returns the smallest integer `next_multiple` greater than `start`
/// such that `next_multiple % factor == 0`.
///
/// # Arguments
/// * `factor` - The multiplication factor. Must be non-zero.
/// * `start` - The starting integer. Must be non-zero.
///
pub fn find_next_multiple(factor: u32, start: u32) -> u32 {
    assert_ne!(factor, 0, "factor must be non-zero");
    assert_ne!(start, 0, "start must be non-zero");

    let remainder = start % factor;
    if remainder == 0 {
        start + factor // If `start` is already a multiple of `factor`, go to the next one.
    } else {
        start + (factor - remainder) // Add the difference to get to the next multiple.
    }
}

/// Zero-pads slice to the left up to length l.
pub fn left_pad_bytes(data: &[u8], l: usize) -> Vec<u8> {
    match l.cmp(&data.len()) {
        std::cmp::Ordering::Greater => {
            let mut padded = vec![0; l - data.len()];
            padded.extend_from_slice(data);
            padded
        }
        _ => data.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    pub fn test_find_next_multiple_of() {
        assert_eq!(find_next_multiple(192, 190), 192);
        assert_eq!(find_next_multiple(8, 10), 16);
        assert_eq!(find_next_multiple(8, 8), 16);
        assert_eq!(find_next_multiple(256, 1000), 1024);
    }

    #[test]
    fn smoke_test_adjust_vector_length() {
        // Test case: Lengthening a vector
        let mut vec = vec![1, 2, 3];
        adjust_vector_length(&mut vec, 5);
        assert_eq!(vec, vec![1, 2, 3, 0, 0]);

        // Test case: Truncating a vector
        let mut vec = vec![1, 2, 3, 4, 5];
        adjust_vector_length(&mut vec, 3);
        assert_eq!(vec, vec![1, 2, 3]);

        // Test case: No change to the vector
        let mut vec = vec![1, 2, 3];
        adjust_vector_length(&mut vec, 3);
        assert_eq!(vec, vec![1, 2, 3]);

        // Edge case: Adjusting an empty vector
        let mut vec: Vec<u8> = Vec::new();
        adjust_vector_length(&mut vec, 3);
        assert_eq!(vec, vec![0, 0, 0]);

        // Edge case: Required length is zero
        let mut vec = vec![1, 2, 3];
        adjust_vector_length(&mut vec, 0);
        assert!(vec.is_empty());
    }

    #[test]
    fn smoke_test_left_pad_bytes() {
        // Test case: Padding is required
        let data = vec![1, 2, 3];
        let padded = left_pad_bytes(&data, 5);
        assert_eq!(padded, vec![0, 0, 1, 2, 3]);

        // Test case: No padding required (length matches)
        let data = vec![1, 2, 3];
        let padded = left_pad_bytes(&data, 3);
        assert_eq!(padded, vec![1, 2, 3]);

        // Test case: No padding required (length shorter)
        let data = vec![1, 2, 3, 4, 5];
        let padded = left_pad_bytes(&data, 3);
        assert_eq!(padded, vec![1, 2, 3, 4, 5]);

        // Edge case: Empty input with padding required
        let data: Vec<u8> = Vec::new();
        let padded = left_pad_bytes(&data, 4);
        assert_eq!(padded, vec![0, 0, 0, 0]);

        // Edge case: No padding required for empty input and length 0
        let data: Vec<u8> = Vec::new();
        let padded = left_pad_bytes(&data, 0);
        assert_eq!(padded, Vec::<u8>::new());
    }
}
