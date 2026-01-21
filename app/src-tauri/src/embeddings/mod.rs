pub mod cohere;
pub mod fireworks;
pub mod openai;

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a <= 0.0 || norm_b <= 0.0 {
        return None;
    }

    Some(dot / (norm_a.sqrt() * norm_b.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "identical vectors should have similarity 1.0"
        );
    }

    #[test]
    fn cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "opposite vectors should have similarity -1.0"
        );
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            sim.abs() < 1e-6,
            "orthogonal vectors should have similarity 0.0"
        );
    }

    #[test]
    fn cosine_similarity_mismatched_lengths() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0];
        assert!(cosine_similarity(&a, &b).is_none());
    }

    #[test]
    fn cosine_similarity_empty_vectors() {
        let empty: Vec<f32> = vec![];
        assert!(cosine_similarity(&empty, &empty).is_none());
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let zero = vec![0.0, 0.0, 0.0];
        let non_zero = vec![1.0, 2.0, 3.0];
        assert!(cosine_similarity(&zero, &non_zero).is_none());
    }

    #[test]
    fn cosine_similarity_similar_direction() {
        // Two vectors pointing in roughly the same direction.
        let a = vec![1.0, 1.0];
        let b = vec![2.0, 2.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "parallel vectors should have similarity 1.0 regardless of magnitude"
        );
    }

    #[test]
    fn cosine_similarity_realistic_embeddings() {
        // Simulated embedding vectors (3 dimensions for simplicity).
        // "hello" and "hi" should be similar; "hello" and "goodbye" less so.
        let hello = vec![0.9, 0.1, 0.0];
        let hi = vec![0.85, 0.15, 0.05];
        let goodbye = vec![0.1, 0.9, 0.0];

        let sim_hello_hi = cosine_similarity(&hello, &hi).unwrap();
        let sim_hello_goodbye = cosine_similarity(&hello, &goodbye).unwrap();

        assert!(
            sim_hello_hi > sim_hello_goodbye,
            "similar words should have higher similarity"
        );
        assert!(sim_hello_hi > 0.9, "similar words should be highly similar");
    }
}
