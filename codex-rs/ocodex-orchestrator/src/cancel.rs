use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn is_canceled(&self) -> bool { self.0.load(Ordering::Relaxed) }
}

#[derive(Clone, Default)]
pub struct CancelSource(Arc<AtomicBool>);

impl CancelSource {
    pub fn token(&self) -> CancelToken { CancelToken(self.0.clone()) }
    pub fn cancel(&self) { self.0.store(true, Ordering::Relaxed); }
}

