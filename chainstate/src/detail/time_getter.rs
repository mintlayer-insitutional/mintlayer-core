use common::primitives::time;
use std::sync::Arc;

pub type TimeGetterFn = dyn Fn() -> i64 + Send + Sync;

/// A function wrapper that contains the function that will be used to get the current time in chainstate
pub struct TimeGetter {
    f: Arc<TimeGetterFn>,
}

impl TimeGetter {
    pub fn new(f: Arc<TimeGetterFn>) -> Self {
        Self { f }
    }

    pub fn get_time(&self) -> i64 {
        (self.f)()
    }

    pub fn getter(&self) -> &TimeGetterFn {
        &*self.f
    }
}

impl Default for TimeGetter {
    fn default() -> Self {
        Self::new(Arc::new(time::get))
    }
}