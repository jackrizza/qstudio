mod aluminum;
mod copper;

pub use aluminum::Aluminum;
pub use copper::Copper;

use crossbeam_channel::Sender;

pub trait Unravel<A, T, B> {
    fn get_type(&self) -> A
    where
        A: std::fmt::Debug;
    fn do_something(&self, tx: Sender<T>) -> B;
}

pub trait Response<A, B> {
    fn message(&self) -> String;
    fn default() -> Self
    where
        Self: Sized;
    fn event_type(&self) -> A;
}

pub trait MakeT<T> {
    fn make_t(&self) -> T;
}
