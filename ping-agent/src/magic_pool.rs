pub use poule::{Checkout, Pool};
pub use std::ops::DerefMut;

pub struct MagicPool<T: Default + Clone> {
    grow_factor: usize,
    pool: Pool<T>,
}

impl<T: Default + Clone> MagicPool<T> {
    pub fn new(grow_factor: usize) -> Self {
        Self::with_cappacity(grow_factor * 10, grow_factor)
    }

    pub fn with_cappacity(capacity: usize, grow_factor: usize) -> Self {
        MagicPool {
            grow_factor,
            pool: Pool::with_capacity(capacity),
        }
    }

    pub fn get(&mut self) -> Checkout<T> {
        if self.pool.used() == self.pool.capacity() {
            self.pool.grow_to(self.pool.capacity() + self.grow_factor);
        }

        self.pool
            .checkout(T::default)
            .expect("should have a checkout after grow")
    }
}
