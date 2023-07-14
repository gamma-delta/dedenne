use std::{future::Future, pin::Pin};

use crate::{Generator, GeneratorResponse, YieldWrapper};

/// Iterate over a generator.
///
/// You can use this either for generators that have a unit `Q`, or a non-unit `Q`.
//
/// For unit `Q`s, it continually calls `generator.query(())` and returns its `Y`ielded
/// values until the generator runs out.
///
/// For non-unit `Q`s, you have to give it an inner iterator of `Q`s to iterate over.
/// It will get `Q`s from the inner iterator and feed it to the generator until either the
/// inner iterator runs out of `Q`s, or the generator returns its `R`.
///
/// If you want the `R` at the end, you can call `consume_response` or `try_consume_response`.
pub struct GeneratorIterator<Y, R, Q, I> {
  inner: GeneratorIteratorState<Y, R, Q, I>,
}

impl<Y, R, Q, I> GeneratorIterator<Y, R, Q, I> {
  pub(crate) fn new(inner: GeneratorIteratorState<Y, R, Q, I>) -> Self {
    Self { inner }
  }

  /// If the inner generator ever responded, return the response.
  /// Otherwise return `None`.
  pub fn consume_response(self) -> Option<R> {
    match self.inner {
      GeneratorIteratorState::GeneratorDone(response, _) => Some(response),
      _ => None,
    }
  }
}

impl<Y, R, Q, I> Iterator for GeneratorIterator<Y, R, Q, I>
where
  I: Iterator<Item = Q>,
  Y: 'static,
  Q: 'static,
  R: 'static,
{
  type Item = Y;

  fn next(&mut self) -> Option<Self::Item> {
    match &self.inner {
      GeneratorIteratorState::NoInitStart(..) => {
        // that's ergonomic
        let (maker, iter) = match std::mem::replace(
          &mut self.inner,
          GeneratorIteratorState::TmpDodgeBorrowck,
        ) {
          GeneratorIteratorState::NoInitStart(a, b) => (a, b),
          _ => unreachable!(),
        };

        let (started, resp) = Generator::run(maker);

        match resp {
          GeneratorResponse::Yielding(yielded) => {
            self.inner = GeneratorIteratorState::Running(started, iter);
            Some(yielded)
          }
          GeneratorResponse::Done(result) => {
            self.inner = GeneratorIteratorState::GeneratorDone(result, iter);
            None
          }
        }
      }

      GeneratorIteratorState::Running(..) => {
        let (mut generator, mut iter) = match std::mem::replace(
          &mut self.inner,
          GeneratorIteratorState::TmpDodgeBorrowck,
        ) {
          GeneratorIteratorState::Running(a, b) => (a, b),
          _ => unreachable!(),
        };

        let Some(next) = iter.next() else {
          self.inner = GeneratorIteratorState::ExhaustedIterator(generator);
          return None;
        };
        let gen_response = generator.query(next);
        match gen_response {
          GeneratorResponse::Yielding(yielded) => {
            self.inner = GeneratorIteratorState::Running(generator, iter);
            Some(yielded)
          }
          GeneratorResponse::Done(result) => {
            self.inner = GeneratorIteratorState::GeneratorDone(result, iter);
            None
          }
        }
      }
      GeneratorIteratorState::ExhaustedIterator(_)
      | GeneratorIteratorState::GeneratorDone(_, _)
      | GeneratorIteratorState::TmpDodgeBorrowck => None,
    }
  }
}

/// Tracks the state of the `GeneratorIterator`
pub(crate) enum GeneratorIteratorState<Y, R, Q, I> {
  NoInitStart(
    Box<dyn FnOnce(YieldWrapper<Q, Y>) -> Pin<Box<dyn Future<Output = R>>>>,
    I,
  ),
  /// We are still in normal operation
  Running(Generator<Y, R, Q>, I),
  /// The inner iterator ran out
  ExhaustedIterator(Generator<Y, R, Q>),
  /// The outer generator ran out
  GeneratorDone(R, I),

  TmpDodgeBorrowck,
}

impl<Y, R, Q, I> GeneratorIteratorState<Y, R, Q, I>
where
  I: Iterator<Item = Q>,
{
  pub(crate) fn self_start<F, Fut>(f: F, iter: I) -> Self
  where
    F: FnOnce(YieldWrapper<Q, Y>) -> Fut + 'static,
    Fut: Future<Output = R> + 'static,
    Q: 'static,
    Y: 'static,
  {
    let clo = |y: YieldWrapper<Q, Y>| {
      let fut = f(y);
      Box::pin(fut) as Pin<Box<dyn Future<Output = R>>>
    };
    Self::NoInitStart(Box::new(clo) as _, iter)
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn exhaust_inner() {
    let iterator = 0u32..10;

    let mut geniterator =
      Generator::jumpstart_iter_over(iterator, |y| async move {
        let mut acc = 1u32;
        // This loop can only run 10 times
        for i in 0..20 {
          acc *= 2;
          let got = y.ield(acc * 2).await;
          assert_eq!(got, i);
        }
        panic!("last i checked 20 > 10")
      });

    for x in &mut geniterator {
      assert!(x.is_power_of_two());
    }

    assert!(matches!(
      geniterator.inner,
      GeneratorIteratorState::ExhaustedIterator(..)
    ));
  }

  #[test]
  fn finish_generator() {
    // This iterator will only .next 10 elements
    let iterator =
      (0u32..20).chain(std::iter::from_fn(|| panic!("last i checked 20 > 10")));

    let mut geniterator =
      Generator::jumpstart_iter_over(iterator, |y| async move {
        let mut acc = 1u32;
        for i in 0..10 {
          acc *= 2;
          let got = y.ield(acc).await;
          assert_eq!(got, i);
        }
        "All done!"
      });

    for x in &mut geniterator {
      assert!(x.is_power_of_two());
    }

    assert!(matches!(
      geniterator.inner,
      GeneratorIteratorState::GeneratorDone("All done!", _)
    ));
  }
}
