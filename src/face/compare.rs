use super::embed::Embedding;

/// Cosine similarity between two embeddings
/// 1.0 = same person, 0.0 = completely different
pub fn cosine_similarity(a: &Embedding, b: &Embedding) -> f32 {
    let dot: f32 = a.0.iter().zip(b.0.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.0.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.0.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

pub fn is_match(a: &Embedding, b: &Embedding, threshold: f32) -> bool {
    cosine_similarity(a, b) >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::embed::Embedding;

    fn emb(v: f32, n: usize) -> Embedding {
        Embedding(vec![v; n])
    }

    #[test]
    fn identical_embeddings_similarity_is_one() {
        let e = emb(1.0, 512);
        let sim = cosine_similarity(&e, &e);
        assert!((sim - 1.0).abs() < 1e-5, "sim={sim}");
    }

    #[test]
    fn zero_embedding_returns_zero() {
        let a = emb(0.0, 512);
        let b = emb(1.0, 512);
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn opposite_embeddings_similarity_is_minus_one() {
        let a = emb(1.0, 512);
        let b = emb(-1.0, 512);
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-5, "sim={sim}");
    }
}
