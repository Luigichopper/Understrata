use serde::{Deserialize, Serialize};
use us_core::CHUNK_DIM;

pub const CHUNK_DIM_USIZE: usize = CHUNK_DIM as usize;
pub const CHUNK_VOLUME: usize = CHUNK_DIM_USIZE * CHUNK_DIM_USIZE * CHUNK_DIM_USIZE; // 32,768

/// Chunk storage field: either a single uniform value across the entire chunk,
/// or a densely allocated array of 32³ elements (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Field<T: Copy + PartialEq> {
    Uniform(T),
    Dense(Box<[T]>),
}

impl<T: Copy + PartialEq> Field<T> {
    /// Creates a uniform field filled with `val`. Zero heap allocations.
    #[inline]
    pub const fn new_uniform(val: T) -> Self {
        Self::Uniform(val)
    }

    /// Linear index from 3D local coordinates (0..32).
    #[inline]
    pub const fn index(x: usize, y: usize, z: usize) -> usize {
        x + CHUNK_DIM_USIZE * (y + CHUNK_DIM_USIZE * z)
    }

    /// Reads the value at local coordinate (x, y, z).
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> T {
        match self {
            Self::Uniform(v) => *v,
            Self::Dense(data) => data[Self::index(x, y, z)],
        }
    }

    /// Reads the value at linear index (0..32,768).
    #[inline]
    pub fn get_linear(&self, idx: usize) -> T {
        match self {
            Self::Uniform(v) => *v,
            Self::Dense(data) => data[idx],
        }
    }

    /// Sets the value at local coordinate (x, y, z), expanding to dense if necessary.
    pub fn set(&mut self, x: usize, y: usize, z: usize, val: T) {
        let idx = Self::index(x, y, z);
        match self {
            Self::Uniform(curr) => {
                if *curr == val {
                    return;
                }
                let mut data = vec![*curr; CHUNK_VOLUME].into_boxed_slice();
                data[idx] = val;
                *self = Self::Dense(data);
            }
            Self::Dense(data) => {
                data[idx] = val;
            }
        }
    }

    /// Attempts to collapse dense storage back to `Uniform` if all cells are identical.
    pub fn try_collapse(&mut self) -> bool {
        match self {
            Self::Uniform(_) => true,
            Self::Dense(data) => {
                let first = data[0];
                for &v in data.iter().skip(1) {
                    if v != first {
                        return false;
                    }
                }
                *self = Self::Uniform(first);
                true
            }
        }
    }

    /// Returns true if this field is currently collapsed to a uniform value.
    #[inline]
    pub const fn is_uniform(&self) -> bool {
        matches!(self, Self::Uniform(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_uniform_size_and_collapse() {
        assert!(std::mem::size_of::<Field<i8>>() <= 16);

        let mut field: Field<i8> = Field::new_uniform(5);
        assert!(field.is_uniform());
        assert_eq!(field.get(10, 10, 10), 5);

        // Setting a different value expands to dense
        field.set(2, 3, 4, 10);
        assert!(!field.is_uniform());
        assert_eq!(field.get(2, 3, 4), 10);
        assert_eq!(field.get(0, 0, 0), 5);

        // Setting back and collapsing
        field.set(2, 3, 4, 5);
        assert!(field.try_collapse());
        assert!(field.is_uniform());
        assert_eq!(field.get(2, 3, 4), 5);
    }
}
