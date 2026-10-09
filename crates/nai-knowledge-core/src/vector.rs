//! Vector space: index identity, normalization and bounded brute-force top-k.
//! Correct exact retrieval for verification; HNSW-scale latency claims require
//! tested deployments (V3.5 §25.2), not an untested linear scan promise.
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorIdentity {
    pub embedding_provider: String,
    pub model_version: String,
    pub dimension: usize,
    pub normalization: String,
    pub chunker_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VectorError {
    DimensionMismatch,
    UnknownNormalization,
    EmptyVector,
    IndexMismatch,
    Overflow,
}

impl fmt::Display for VectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vector: {:?}", self)
    }
}
impl Error for VectorError {}

#[derive(Debug, Clone)]
pub struct VectorSpace {
    pub identity: VectorIdentity,
    vectors: BTreeMap<String, Vec<f32>>,
}
impl VectorSpace {
    pub fn new(identity: VectorIdentity) -> Self {
        Self {
            identity,
            vectors: BTreeMap::new(),
        }
    }
    pub fn len(&self) -> usize {
        self.vectors.len()
    }
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
    fn normalized(values: &[f32], normalization: &str) -> Result<Vec<f32>, VectorError> {
        if values.is_empty() {
            return Err(VectorError::EmptyVector);
        }
        match normalization {
            "l2" | "cosine" => {
                let norm = values
                    .iter()
                    .map(|v| f64::from(*v).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if !norm.is_finite() || norm == 0.0 {
                    return Err(VectorError::EmptyVector);
                }
                Ok(values.iter().map(|v| (*v as f64 / norm) as f32).collect())
            }
            _ => Err(VectorError::UnknownNormalization),
        }
    }
    pub fn insert(&mut self, chunk_id: &str, values: Vec<f32>) -> Result<(), VectorError> {
        if values.len() != self.identity.dimension {
            return Err(VectorError::DimensionMismatch);
        }
        let normalized = Self::normalized(&values, &self.identity.normalization)?;
        if self.vectors.len() >= 1_000_000 {
            return Err(VectorError::Overflow);
        }
        self.vectors.insert(chunk_id.into(), normalized);
        Ok(())
    }
    /// Bounded exact cosine top-k. Larger candidate sets must use a tested index.
    pub fn top_k(&self, query: &[f32], k: usize) -> Result<Vec<(String, f32)>, VectorError> {
        if query.len() != self.identity.dimension {
            return Err(VectorError::DimensionMismatch);
        }
        let query = Self::normalized(query, &self.identity.normalization)?;
        let k = k.min(256);
        let mut ranked: Vec<(String, f32)> = self
            .vectors
            .iter()
            .map(|(id, values)| {
                let dot = values
                    .iter()
                    .zip(query.iter())
                    .map(|(a, b)| f64::from(*a) * f64::from(*b))
                    .sum::<f64>();
                (id.clone(), dot as f32)
            })
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(k);
        Ok(ranked)
    }
    pub fn get(&self, chunk_id: &str) -> Option<&[f32]> {
        self.vectors.get(chunk_id).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> VectorIdentity {
        VectorIdentity {
            embedding_provider: "test".into(),
            model_version: "v1".into(),
            dimension: 3,
            normalization: "l2".into(),
            chunker_version: "chunk-1".into(),
        }
    }

    #[test]
    fn different_identities_are_isolated_by_the_caller() {
        let mut a = VectorSpace::new(identity());
        a.insert("x", vec![1.0, 0.0, 0.0]).unwrap();
        assert!(a.get("x").is_some());
    }

    #[test]
    fn dimension_mismatch_and_bad_normalization_are_rejected() {
        let mut space = VectorSpace::new(identity());
        assert!(space.insert("x", vec![1.0, 0.0]).is_err());
        assert!(
            VectorSpace::new(VectorIdentity {
                normalization: "unknown".into(),
                ..identity()
            })
            .insert("x", vec![1.0, 0.0, 0.0])
            .is_err()
        );
    }

    #[test]
    fn cosine_top_k_ranks_most_similar_first_and_is_bounded() {
        let mut space = VectorSpace::new(identity());
        space.insert("a", vec![1.0, 0.0, 0.0]).unwrap();
        space.insert("b", vec![0.95, 0.1, 0.0]).unwrap();
        space.insert("c", vec![0.0, 1.0, 0.0]).unwrap();
        let top = space.top_k(&[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "a");
        assert_eq!(top[1].0, "b");
        assert!(top[0].1 >= top[1].1);
    }
}
