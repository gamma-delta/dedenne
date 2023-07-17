#![doc = include_str!("../README.md")]

mod futuring;
pub mod iter;
pub mod wrapper;
pub use wrapper::Generator;

use std::{cell::RefCell, future::Future, pin::Pin, sync::Arc};

use futuring::YieldedFuture;
use iter::{GeneratorIterator, GeneratorIteratorState};

/// Wraps an async function into something that can be used as a generator.
///
/// * `S` is the Start type. This is what you pass in to start the generator.
/// * `Y` is the Yield type. The generator returns this if it's not done yet.
/// * `R` is the Return type. This is what the generator returns when done.
/// * `Q` is the Query type. This is what you pass to the generator to do the next step.
///   By default this is the unit type `()`.
pub struct StartedGenerator<Y, R, Q = ()> {
  gen_func: Pin<Box<dyn Future<Output = R>>>,
  swap_slot: SwapSpaceSlot<Q, Y>,
}

impl<Y, R, Q> StartedGenerator<Y, R, Q> {
  /**
  Create and start a generator.

  The customary way to call this function is
  ```rust
  # use dedenne::*;
  # let foo = ();
  # let _: (StartedGenerator<u32, &'static str>, _) =
  StartedGenerator::run_with(foo, |y, foo| async move {
    y.ield(1).await;
    y.ield(2).await;
    y.ield(3).await;
    "All done!"
  });
  ```

  In other words, it's a closure that takes an argument `y` and
  immediately enters an `async` block.
  The `move` is required because currently rustc doesn't like non-`move` async
  blocks.

  `y` is of type [`YieldWrapper`]. a struct that defines an `.ield(Y)` function.
  Laugh at me all you want, it works.
  `.ield` returns a future that exits control flow out to the user,
  and goes back to the closure once `[Generator::query]` is called.
  */
  pub fn run_with<S, F, Fut>(start: S, f: F) -> (Self, GeneratorResponse<Y, R>)
  where
    F: FnOnce(YieldWrapper<Q, Y>, S) -> Fut,
    Fut: Future<Output = R> + 'static,
    Q: 'static,
    Y: 'static,
  {
    let state = Arc::new(RefCell::new(SwapSpace::JustStarted));
    let y = YieldWrapper::new(state.clone());
    let fut = f(y, start);

    let mut me = Self {
      gen_func: Box::pin(fut),
      swap_slot: state,
    };
    // Must step immediately because the user needs to `query` to get a response out otherwise
    let out = me.step_generator();
    (me, out)
  }

  /// `run_with` a unit start
  pub fn run<F, Fut>(f: F) -> (Self, GeneratorResponse<Y, R>)
  where
    F: FnOnce(YieldWrapper<Q, Y>) -> Fut + 'static,
    Fut: Future<Output = R> + 'static,
    Q: 'static,
    Y: 'static,
  {
    let inner_fut = |y, ()| async move { f(y).await };
    StartedGenerator::run_with::<(), _, _>((), inner_fut)
  }

  pub fn query(&mut self, query: Q) -> GeneratorResponse<Y, R> {
    let mut lock = self.swap_slot.borrow_mut();
    match std::mem::replace(&mut *lock, SwapSpace::GotQuery(query)) {
      SwapSpace::WaitingForQuery => {} // all good
      SwapSpace::Finished => {
        panic!("Tried to query a generator after it had finished")
      }
      ono => unreachable!(
        "Tried to query a generator while in {:?}, an illegal state",
        &ono
      ),
    };
    drop(lock);

    self.step_generator()
  }

  /// Create an iterator that repeatedly feeds another iterator into this.
  /// In order to call this method the iterator needs to have already been started.
  ///
  /// See [`GeneratorIterator`].
  pub fn iter_over<I>(self, iter: I) -> GeneratorIterator<Y, R, Q, I> {
    GeneratorIterator::new(GeneratorIteratorState::Running(self, iter))
  }

  pub fn jumpstart_iter_over<I, F, Fut>(
    iter: I,
    f: F,
  ) -> GeneratorIterator<Y, R, Q, I>
  where
    F: FnOnce(YieldWrapper<Q, Y>) -> Fut + 'static,
    Fut: Future<Output = R> + 'static,
    I: Iterator<Item = Q>,
    Q: 'static,
    Y: 'static,
  {
    GeneratorIterator::new(GeneratorIteratorState::self_start(f, iter))
  }

  fn step_generator(&mut self) -> GeneratorResponse<Y, R> {
    let result = futuring::resume(&mut self.gen_func);
    let mut lock = self.swap_slot.borrow_mut();
    if let Some(finished) = result {
      match std::mem::replace(&mut *lock, SwapSpace::Finished) {
        // we are "processing" it because we aren't able to call the code that says
        // we're finished (?)
        SwapSpace::ProcessingQuery => GeneratorResponse::Done(finished),
        ono => {
          unreachable!(
            "When the closure returned, was in illegal state {:?}",
            &ono
          )
        }
      }
    } else {
      match std::mem::replace(&mut *lock, SwapSpace::WaitingForQuery) {
        SwapSpace::Yielding(y) => GeneratorResponse::Yielding(y),
        ono => {
          unreachable!(
            "When the closure yielded, was in illegal state {:?}",
            &ono
          )
        }
      }
    }
  }
}

impl<Y, R> StartedGenerator<Y, R, ()> {
  /// Convenience wrapper for `query(())`, or querying with a unit.
  pub fn resume(&mut self) -> GeneratorResponse<Y, R> {
    self.query(())
  }

  /// Create an iterator that repeatedly feeds () into this.
  pub fn iter(self) -> GeneratorIterator<Y, R, (), std::iter::Repeat<()>> {
    self.iter_over(std::iter::repeat(()))
  }

  pub fn jumpstart_iter<F, Fut>(
    f: F,
  ) -> GeneratorIterator<Y, R, (), std::iter::Repeat<()>>
  where
    F: FnOnce(YieldWrapper<(), Y>) -> Fut + 'static,
    Fut: Future<Output = R> + 'static,
    Y: 'static,
  {
    StartedGenerator::jumpstart_iter_over(std::iter::repeat(()), f)
  }
}

/// The result of querying a generator.
/// Either it will `Y`ield a value, or be done and return a `R`esponse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratorResponse<Y, R> {
  Yielding(Y),
  Done(R),
}

/// The type of `y` in `y.ield(foo)`.
pub struct YieldWrapper<Q, Y> {
  swap_slot: SwapSpaceSlot<Q, Y>,
}

impl<Q, Y> YieldWrapper<Q, Y> {
  pub(crate) fn new(swap_slot: SwapSpaceSlot<Q, Y>) -> Self {
    Self { swap_slot }
  }

  /// Call this as `y.ield`. It returns a future that returns your querying type.
  /// Control flow will return to the inner closure once the user calls `generator.query`
  pub fn ield(&self, yielded: Y) -> impl Future<Output = Q> {
    YieldedFuture::new(self.swap_slot.clone(), yielded)
  }
}

#[derive(derive_debug::Dbg)]
enum SwapSpace<Q, Y> {
  /// Freshly created
  JustStarted,
  /// The user has *just* called `generator.query()`.
  /// Will remain in this state for a very small amount of time, in the interim period where
  /// the user has submitted a query but the generator is still routing the data around before it
  /// calls self.step_generator().
  GotQuery(#[dbg(placeholder = "<Q>")] Q),
  /// Control flow is now *inside* the closure. We are now waiting for the closure to call `y.ield(foo)`.
  ProcessingQuery,
  /// Closure has called `y.ield(foo)`, which puts `foo` in here
  Yielding(#[dbg(placeholder = "<Y>")] Y),
  /// The user has retrieved the yielded value.
  WaitingForQuery,
  /// Querying again is an error now.
  Finished,
}

type SwapSpaceSlot<Q, Y> = Arc<RefCell<SwapSpace<Q, Y>>>;
