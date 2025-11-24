//! Batching utilities for memory-efficient operations
//!
//! Provides Django-style batching for large querysets to prevent OOM errors.

/// Default batch size for bulk operations
pub const DEFAULT_BATCH_SIZE: usize = 1000;

/// Maximum batch size allowed (safety guard)
pub const MAX_BATCH_SIZE: usize = 10_000;

/// Chunk size for iterator streaming
pub const DEFAULT_CHUNK_SIZE: usize = 100;

/// Validates batch size and returns safe value
pub fn validate_batch_size(size: Option<usize>) -> usize {
    match size {
        Some(0) => DEFAULT_BATCH_SIZE,
        Some(s) if s > MAX_BATCH_SIZE => {
            eprintln!(
                "Warning: batch_size {s} exceeds maximum {MAX_BATCH_SIZE}. Using {MAX_BATCH_SIZE}"
            );
            MAX_BATCH_SIZE
        }
        Some(s) => s,
        None => DEFAULT_BATCH_SIZE,
    }
}

/// Split a vector into batches
pub fn batch_vec<T: Clone>(items: Vec<T>, batch_size: usize) -> Vec<Vec<T>> {
    let batch_size = validate_batch_size(Some(batch_size));

    items
        .into_iter()
        .collect::<Vec<_>>()
        .chunks(batch_size)
        .map(<[T]>::to_vec)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_batch_size() {
        assert_eq!(validate_batch_size(None), DEFAULT_BATCH_SIZE);
        assert_eq!(validate_batch_size(Some(0)), DEFAULT_BATCH_SIZE);
        assert_eq!(validate_batch_size(Some(500)), 500);
        assert_eq!(validate_batch_size(Some(15000)), MAX_BATCH_SIZE);
    }

    #[test]
    fn test_batch_vec() {
        let items: Vec<i32> = (1..=10).collect();
        let batches = batch_vec(items, 3);

        assert_eq!(batches.len(), 4); // 3, 3, 3, 1
        assert_eq!(batches[0], vec![1, 2, 3]);
        assert_eq!(batches[3], vec![10]);
    }
}
