//! Defines the convenience `Generator` wrapper.
//! Just makes you need less horrible tuple destructuring.
//!
//! The main docs for how this crate actually works are on
//! the structs in the root.

use std::future::Future;

use crate::{
  GeneratorResponse, StartedGenerator, UnstartedGenerator, YieldWrapper,
};

/// Silly convenience wrapper over a started or unstarted generator.
///
/// This just makes things more convenient to call,
/// instead of awkward tuple destructuring.
///
/// Because this is just a convenience wrapper the main docs for things
/// are on the inner types. Go check those out.
pub struct Generator<S, Y, R, Q = ()> {
  inner: GeneratorWrapperInner<S, Y, R, Q>,
}

/// Inner `Either`-like enum for the generator wrapper.
pub enum GeneratorWrapperInner<S, Y, R, Q> {
  Unstarted(UnstartedGenerator<S, Y, R, Q>),
  TheSplitSecondBetweenNotStartedAndStarted,
  Started(StartedGenerator<Y, R, Q>),
}

impl<S, Y, R, Q> Generator<S, Y, R, Q> {
  pub fn wrap<F, Fut>(f: F) -> Self
  where
    F: FnOnce(YieldWrapper<Q, Y>) -> Fut,
    Fut: Future<Output = R> + 'static,
    Q: 'static,
    Y: 'static,
  {
    let unstarted = UnstartedGenerator::wrap(f);
    Self {
      inner: GeneratorWrapperInner::Unstarted(unstarted),
    }
  }

  pub fn start(&mut self, init: S) -> GeneratorResponse<Y, R> {
    match self.inner {
      GeneratorWrapperInner::Unstarted(..) => {
        // aaugh
        let swapped = std::mem::replace(
          &mut self.inner,
          GeneratorWrapperInner::TheSplitSecondBetweenNotStartedAndStarted,
        );
        let unstarted = match swapped {
          GeneratorWrapperInner::Unstarted(it) => it,
          _ => unreachable!(),
        };
        let (started, out) = unstarted.start();
        self.inner = GeneratorWrapperInner::Started(started);
        out
      }
      GeneratorWrapperInner::Started(..) => {
        panic!("don't start a generator you've already started!")
      }
      GeneratorWrapperInner::TheSplitSecondBetweenNotStartedAndStarted => {
        unreachable!()
      }
    }
  }

  pub fn query(&mut self, query: Q) -> GeneratorResponse<Y, R> {
    match self.inner {
      GeneratorWrapperInner::Started(ref mut started) => started.query(query),
      GeneratorWrapperInner::Unstarted(..) => {
        panic!("don't query a Generator you haven't started!")
      }
      GeneratorWrapperInner::TheSplitSecondBetweenNotStartedAndStarted => {
        unreachable!()
      }
    }
  }

  fn has_started(&self) -> bool {
    match self.inner {
      GeneratorWrapperInner::Unstarted(_)
      | GeneratorWrapperInner::TheSplitSecondBetweenNotStartedAndStarted => {
        false
      }
      GeneratorWrapperInner::Started(_) => true,
    }
  }
}

impl<Y, R> Generator<Y, R, ()> {
  pub fn resume(&mut self) -> GeneratorResponse<Y, R> {
    self.query(())
  }
}
