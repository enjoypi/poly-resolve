use core::hash::{BuildHasher as _, Hasher as _};
use std::hash::RandomState;

pub(crate) fn spread(wait: u64) -> u64 {
    let random = RandomState::new().build_hasher().finish();
    wait.saturating_sub(wait >> 2)
        .saturating_add((wait >> 1) & random)
}
