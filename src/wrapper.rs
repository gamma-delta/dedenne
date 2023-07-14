//! Defines the convenience `Generator` wrapper.
//! Just makes you need less horrible tuple destructuring.
//!
//! The main docs for how this crate actually works are on
//! the structs in the root.

use std::{future::Future, marker::PhantomData};

use crate::{Generator, GeneratorResponse, YieldWrapper};

/// Silly convenience wrapper over a started or unstarted generator.
///
/// This just makes things more convenient to call,
/// instead of awkward tuple destructuring.
///
/// I can stop using generics whenever I want
pub struct GeneratorWrapper<F, Fut, S, Y, R, Q = ()> {
  inner: GeneratorWrapperInner<F, Fut, S, Y, R, Q>,
}

/// Inner `Either`-like enum for the generator wrapper.
pub enum GeneratorWrapperInner<F, Fut, S, Y, R, Q> {
  Unstarted {
    future_maker: F,
    _phantom: PhantomData<(Fut, S)>,
  },
  Starting,
  Started(Generator<Y, R, Q>),
}

impl<F, Fut, S, Y, R, Q> GeneratorWrapper<F, Fut, S, Y, R, Q>
where
  F: FnOnce(YieldWrapper<Q, Y>, S) -> Fut,
  Fut: Future<Output = R> + 'static,
  Q: 'static,
  Y: 'static,
{
  /// Doesn't start anything yet
  pub fn new(f: F) -> Self {
    let inner = GeneratorWrapperInner::Unstarted {
      future_maker: f,
      _phantom: PhantomData,
    };
    Self { inner }
  }

  pub fn start(&mut self, init: S) -> GeneratorResponse<Y, R> {
    match self.inner {
      GeneratorWrapperInner::Unstarted { .. } => {
        // aaugh
        let swapped =
          std::mem::replace(&mut self.inner, GeneratorWrapperInner::Starting);
        let future_maker = match swapped {
          GeneratorWrapperInner::Unstarted { future_maker, .. } => future_maker,
          _ => unreachable!(),
        };
        let (started, out) = Generator::run_with(init, future_maker);
        self.inner = GeneratorWrapperInner::Started(started);
        out
      }
      GeneratorWrapperInner::Started(..) => {
        panic!("don't start a generator you've already started!")
      }
      GeneratorWrapperInner::Starting => {
        unreachable!()
      }
    }
  }

  pub fn query(&mut self, query: Q) -> GeneratorResponse<Y, R> {
    match self.inner {
      GeneratorWrapperInner::Started(ref mut started) => started.query(query),
      GeneratorWrapperInner::Unstarted { .. } => {
        panic!("don't query a Generator you haven't started!")
      }
      GeneratorWrapperInner::Starting => {
        unreachable!()
      }
    }
  }

  pub fn has_started(&self) -> bool {
    match self.inner {
      GeneratorWrapperInner::Unstarted { .. }
      | GeneratorWrapperInner::Starting => false,
      GeneratorWrapperInner::Started(_) => true,
    }
  }
}

impl<F, Fut, S, Y, R> GeneratorWrapper<F, Fut, S, Y, R, ()>
where
  F: FnOnce(YieldWrapper<(), Y>, S) -> Fut,
  Fut: Future<Output = R> + 'static,
  Y: 'static,
{
  pub fn resume(&mut self) -> GeneratorResponse<Y, R> {
    self.query(())
  }
}
