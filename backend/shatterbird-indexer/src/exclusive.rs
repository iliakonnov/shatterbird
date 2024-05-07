// #![feature(exclusive_wrapper)]
// Tracking issue: https://github.com/rust-lang/rust/issues/98407
// Source: https://github.com/rust-lang/rust/blob/master/library/core/src/sync/exclusive.rs

//! Defines [`Exclusive`].

use core::fmt;

/// `Exclusive` provides only _mutable_ access, also referred to as _exclusive_
/// access to the underlying value. It provides no _immutable_, or _shared_
/// access to the underlying value.
///
/// While this may seem not very useful, it allows `Exclusive` to _unconditionally_
/// implement [`Sync`]. Indeed, the safety requirements of `Sync` state that for `Exclusive`
/// to be `Sync`, it must be sound to _share_ across threads, that is, it must be sound
/// for `&Exclusive` to cross thread boundaries. By design, a `&Exclusive` has no API
/// whatsoever, making it useless, thus harmless, thus memory safe.
///
/// ## Examples
/// Using a non-`Sync` future prevents the wrapping struct from being `Sync`
/// ```compile_fail
/// use core::cell::Cell;
///
/// async fn other() {}
/// fn assert_sync<T: Sync>(t: T) {}
/// struct State<F> {
///     future: F
/// }
///
/// assert_sync(State {
///     future: async {
///         let cell = Cell::new(1);
///         let cell_ref = &cell;
///         other().await;
///         let value = cell_ref.get();
///     }
/// });
/// ```
// `Exclusive` can't have `PartialOrd`, `Clone`, etc. impls as they would
// use `&` access to the inner value, violating the `Sync` impl's safety
// requirements.
#[derive(Default)]
#[repr(transparent)]
pub struct Exclusive<T: ?Sized> {
    inner: T,
}

// See `Exclusive`'s docs for justification.
unsafe impl<T: ?Sized> Sync for Exclusive<T> {}

impl<T: ?Sized> fmt::Debug for Exclusive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Exclusive").finish_non_exhaustive()
    }
}

impl<T: Sized> Exclusive<T> {
    #[must_use]
    #[inline]
    pub const fn new(t: T) -> Self {
        Self { inner: t }
    }
}

impl<T: ?Sized> Exclusive<T> {
    #[must_use]
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}
