#![doc = include_str!("../README.md")]

mod futuring;
pub mod iter;
pub mod wrapper;

use std::{cell::RefCell, future::Future, pin::Pin, sync::Arc};

use futuring::YieldedFuture;
use iter::GeneratorIterator;
pub use wrapper::Generator;

/// Wraps an async function into something that can be used as a generator.
///
/// * `Y` is the Yield type. The generator returns this if it's not done yet.
/// * `R` is the Return type. This is what the generator returns when done.
/// * `Q` is the Query type. This is what you pass to the generator to do the next step.
///   By default this is the unit type `()`
pub struct UnstartedGenerator<Y, R, Q = ()> {
  /// This is just a type-fucking wrapper tbh
  inner: StartedGenerator<Y, R, Q>,
}

impl<Y, R, Q> UnstartedGenerator<Y, R, Q> {
  /**
  Create a generator from a closure.

  The customary way to call this function is
  ```rust
  # use dedenne::*;
  # let _my_generator: UnstartedGenerator<u32, &'static str> =
  UnstartedGenerator::wrap(|y| async move {
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
  pub fn wrap<F, Fut>(f: F) -> Self
  where
    F: FnOnce(YieldWrapper<Q, Y>) -> Fut,
    Fut: Future<Output = R> + 'static,
    Q: 'static,
    Y: 'static,
  {
    // auugghgh
    let swap_slot = Arc::new(RefCell::new(SwapSpace::Unstarted));
    let yield_maker = YieldWrapper {
      swap_slot: swap_slot.clone(),
    };

    let fut = f(yield_maker);
    let box_fut = Box::pin(fut) as Pin<Box<dyn Future<Output = R>>>;
    Self {
      inner: StartedGenerator {
        gen_func: box_fut,
        swap_slot,
      },
    }
  }

  /// Start the generator. Returns either the first thing given to `y.ield`, or the return value at the end.
  pub fn start(self) -> (StartedGenerator<Y, R, Q>, GeneratorResponse<Y, R>) {
    let mut started = self.inner;
    let result = started.step_generator();
    (started, result)
  }

  /// Create an iterator that repeatedly feeds another iterator into this.
  ///
  /// See [`GeneratorIterator`].
  pub fn iter_over<I>(self, iter: I) -> GeneratorIterator<Y, R, Q, I> {
    let (started, resp) = self.start();
    GeneratorIterator::new(started, iter, resp)
  }
}

impl<Y, R> UnstartedGenerator<Y, R, ()> {
  /// Create an iterator that calls this generator over and over with `()`.
  ///
  /// See [`GeneratorIterator`].
  pub fn iter(self) -> GeneratorIterator<Y, R, (), std::iter::Repeat<()>> {
    self.iter_over(std::iter::repeat(()))
  }
}

pub struct StartedGenerator<Y, R, Q = ()> {
  gen_func: Pin<Box<dyn Future<Output = R>>>,
  swap_slot: SwapSpaceSlot<Q, Y>,
}

impl<Y, R, Q> StartedGenerator<Y, R, Q> {
  /// Convenience function to wrap a closure and start it.
  pub fn start<F, Fut>(f: F) -> (Self, GeneratorResponse<Y, R>)
  where
    F: FnOnce(YieldWrapper<Q, Y>) -> Fut,
    Fut: Future<Output = R> + 'static,
    Q: 'static,
    Y: 'static,
  {
    let unstarted = UnstartedGenerator::wrap(f);
    unstarted.start()
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
}

/// The result of querying a generator.
/// Either it will `Y`ield a value, or be done and return a `R`esponse.
pub enum GeneratorResponse<Y, R> {
  Yielding(Y),
  Done(R),
}

/// The type of `y` in `y.ield(foo)`.
pub struct YieldWrapper<Q, Y> {
  swap_slot: SwapSpaceSlot<Q, Y>,
}

impl<Q, Y> YieldWrapper<Q, Y> {
  /// Call this as `y.ield`. It returns a future that returns your querying type.
  /// Control flow will return to the inner closure once the user calls `generator.query`
  pub fn ield(&self, yielded: Y) -> impl Future<Output = Q> {
    YieldedFuture::new(self.swap_slot.clone(), yielded)
  }
}

#[derive(derive_debug::Dbg)]
enum SwapSpace<Q, Y> {
  /// Freshly created
  Unstarted,
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
