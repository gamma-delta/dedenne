use std::{
  future::Future,
  pin::Pin,
  task::{Poll, RawWaker, RawWakerVTable, Waker},
};

use crate::{SwapSpace, SwapSpaceSlot};

/// This is what `y.ield` returns.
///
/// Calling `y.ield` fills the swap slot with Y, via `SwapSpace::Yielding`.
///
/// Calling `poll` first checks if the swap slot is `SwapSpace::Yielding`; if it is,
/// it's the `poll` originally done by the `await`.
/// Otherwise, it's being called from `Generator::step_generator`, so it should be `GotQuery`,
/// hopefully.
pub(crate) struct YieldedFuture<Q, Y> {
  swap_slot: SwapSpaceSlot<Q, Y>,
}

impl<Q, Y> YieldedFuture<Q, Y> {
  // This function (closed over swap_slot) is the `yielder` function.
  pub fn new(swap_slot: SwapSpaceSlot<Q, Y>, yielded: Y) -> Self {
    // Immediately smuggle out the yielded value
    let lock = swap_slot.borrow();
    let is_yielding = matches!(&*lock, SwapSpace::Yielding(..));
    drop(lock);

    if is_yielding {
      panic!("Found yielding state when making a new YieldedFuture. Be sure to remember the `.await` after!")
    }

    let mut lock = swap_slot.borrow_mut();
    match std::mem::replace(&mut *lock, SwapSpace::Yielding(yielded)) {
      SwapSpace::ProcessingQuery | SwapSpace::JustStarted => {}
      ono => unreachable!(
        "while making a new YieldedFuture, was in the illegal state {:?}",
        &ono
      ),
    }
    drop(lock);

    // Wait until Self::poll is called to smuggle in the Q
    // Therefore it's on me to only ever do so once the user has filled it.
    Self { swap_slot }
  }
}

impl<Q, Y> Future for YieldedFuture<Q, Y> {
  type Output = Q;

  fn poll(
    self: Pin<&mut Self>,
    _cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    let lock = self.swap_slot.borrow();
    match &*lock {
      // Just called y.ield
      SwapSpace::Yielding(_) => std::task::Poll::Pending,
      // Called from step_generator
      SwapSpace::GotQuery(_) => {
        drop(lock);
        let mut lock = self.swap_slot.borrow_mut();
        let query =
          match std::mem::replace(&mut *lock, SwapSpace::ProcessingQuery) {
            SwapSpace::GotQuery(q) => q,
            _ => unreachable!(),
          };
        std::task::Poll::Ready(query)
      }
      ono => unreachable!(
        "Tried to poll the YieldedFuture while in the illegal state {:?}",
        &ono
      ),
    }
  }
}

// https://github.com/not-fl3/macroquad/blob/master/src/exec.rs
fn waker() -> Waker {
  unsafe fn clone(data: *const ()) -> RawWaker {
    RawWaker::new(data, &VTABLE)
  }
  unsafe fn wake(_data: *const ()) {
    panic!(
      "Cannot wake a Dedenne future (are you using this with a runtime that isn't Dedenne like I told you not to?)"
    )
  }
  unsafe fn wake_by_ref(data: *const ()) {
    wake(data)
  }
  unsafe fn drop(_data: *const ()) {
    // Nothing to do
  }
  const VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone, wake, wake_by_ref, drop);
  let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
  unsafe { Waker::from_raw(raw_waker) }
}

/// returns Some(T) if future is done, None if it would block
pub(crate) fn resume<T>(
  future: &mut Pin<Box<dyn Future<Output = T>>>,
) -> Option<T> {
  let waker = waker();
  let mut futures_context = std::task::Context::from_waker(&waker);
  match future.as_mut().poll(&mut futures_context) {
    Poll::Ready(v) => Some(v),
    Poll::Pending => None,
  }
}
