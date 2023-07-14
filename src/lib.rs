#![doc = include_str!("../README.md")]

mod futuring;
pub mod iter;
pub mod wrapper;

use std::{
  cell::RefCell, future::Future, marker::PhantomData, pin::Pin, rc::Rc,
  sync::Arc,
};

use futuring::YieldedFuture;
use iter::GeneratorIterator;
pub use wrapper::Generator;

pub trait GeneratorEngine<S, Y, R, Q> {
  type FutOut: Future<Output = R>;
  fn make_generator(self, y: YieldWrapper<Q, Y>, start: S) -> Self::FutOut;
}

impl<T, S, Y, R, Q, Fut> GeneratorEngine<S, Y, R, Q> for T
where
  T: FnOnce(YieldWrapper<Q, Y>, Q) -> Fut,
  Fut: Future<Output = R>,
{
  type FutOut = Fut;

  fn make_generator(self, y: YieldWrapper<Q, Y>, start: S) -> Self::FutOut {
    (self)(y, start)
  }
}

/// Wraps an async function into something that can be used as a generator.
///
/// * `S` is the Start type. This is what you pass in to start the generator.
/// * `Y` is the Yield type. The generator returns this if it's not done yet.
/// * `R` is the Return type. This is what the generator returns when done.
/// * `Q` is the Query type. This is what you pass to the generator to do the next step.
///   By default this is the unit type `()`.
///
/// This struct is just a kind of staging area. It doesn't do anything until started.
pub struct UnstartedGenerator<Engine, S, Y, R, Q = ()> {
  engine: Engine,
  _phantom: PhantomData<(S, Y, R, Q)>,
}

impl<Engine, S, Y, R, Q, Fut> UnstartedGenerator<Engine, S, Y, R, Q>
where
  Engine: GeneratorEngine<S, Y, R, Q, FutOut = Fut>,
{
  /**
  Create a generator from a [`GeneratorEngine`].

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

  But of cource you can pass any Engine type you want.
    */
  pub fn wrap(engine: Engine) -> Self
  where
    Q: 'static,
    Y: 'static,
  {
    Self {
      engine,
      _phantom: PhantomData,
    }
  }

  /// Start the generator. Returns either the first thing given to `y.ield`, or the return value at the end.
  pub fn start(
    self,
    init: S,
  ) -> (StartedGenerator<Y, R, Q>, GeneratorResponse<Y, R>) {
    StartedGenerator::wrap_and_start(self.engine, init)
  }
}

impl<Engine, S, Y, R, Q> UnstartedGenerator<Engine, S, Y, R, ()> {
  /// Create an iterator that calls this generator over and over with `()`.
  ///
  /// See [`GeneratorIterator`].
  pub fn iter(self) -> GeneratorIterator<Y, R, (), std::iter::Repeat<()>> {
    self.iter_over(std::iter::repeat(()))
  }
}

impl<Engine, S, Y, R, Q, I> UnstartedGenerator<Engine, S, Y, R, Q>
where
  I: Iterator<Item = Q>,
{
  /// Create an iterator that calls this generator over and over.
  ///
  /// See [`GeneratorIterator`].
  pub fn start_iter(self, init: S, iter: I) -> GeneratorIterator<Y, R, Q, I> {
    let (started, first) = self.start(init);
    GeneratorIterator::new(started, i, Some(first))
  }
}

impl<Engine, S, Y, R, I> UnstartedGenerator<Engine, S, Y, R, S>
where
  I: Iterator<Item = Q>,
{
  /// Create an iterator that calls this generator over and over.
  ///
  /// NOTE: This ONLY works when the `S`tart type is the same as the `Q`uery type.
  /// This way we can feed the original element of the iterator in.
  ///
  /// See [`GeneratorIterator`].
  pub fn self_start_iter(self, start: S) -> GeneratorIterator<Y, R, Q, I> {}
}

impl<Engine, S, Y, R, Q> Clone for UnstartedGenerator<Engine, S, Y, R, Q>
where
  Engine: Clone,
{
  fn clone(&self) -> Self {
    Self {
      engine: self.engine.clone(),
      _phantom: PhantomData,
    }
  }
}

pub struct StartedGenerator<Y, R, Q = ()> {
  gen_func: Pin<Box<dyn Future<Output = R>>>,
  swap_slot: SwapSpaceSlot<Q, Y>,
}

impl<S, Y, R, Q> StartedGenerator<Y, R, Q> {
  pub fn wrap_and_start<Engine>(
    engine: Engine,
    start: S,
  ) -> (Self, GeneratorResponse<Y, R>)
  where
    Engine: GeneratorEngine<S, Y, R, Q>,
    Q: 'static,
    Y: 'static,
  {
    let state = Arc::new(Rc::new(SwapSpace::JustStarted));
    let y = YieldWrapper::new(state.clone());
    let fut = engine.make_generator(y, start);

    let mut me = Self {
      gen_func: Pin::new(fut),
      swap_slot: state,
    };
    // Must step immediately because the user needs to `query` to get a response out otherwise
    let out = me.step_generator();
    (me, out)
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
    GeneratorIterator::new(self, iter, None)
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
  pub fn new(swap_slot: SwapSpaceSlot<Q, Y>) -> Self {
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
