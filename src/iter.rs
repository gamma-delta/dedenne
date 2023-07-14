use crate::{Generator, GeneratorResponse, StartedGenerator};

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
///
/// I've exposed the inner fields but you probably won't need them.
pub struct GeneratorIterator<Y, R, Q, I> {
  /// Inner generator. Usually this will be the Started case,
  /// but for self-starting iterators it will be unstarted at first.
  ///
  /// Necessarily has S=Q.
  pub generator: Generator<Q, Y, R, Q>,
  pub iter: I,
  pub start: Option<GeneratorResponse<Y, R>>,
  pub finished: GeneratorIteratorFinish<R>,
}

impl<Y, R, Q, I> GeneratorIterator<Y, R, Q, I> {
  pub fn new(
    generator: Generator<Y, R, Q>,
    iter: I,
    start: Option<GeneratorResponse<Y, R>>,
  ) -> Self {
    Self {
      generator,
      iter,
      start,
      finished: GeneratorIteratorFinish::NotFinished,
    }
  }

  /// If the inner generator ever responded, return the response.
  /// Otherwise return `None`.
  ///
  /// If you want more information, just check on the fields directly.
  pub fn consume_response(self) -> Option<R> {
    match self.finished {
      GeneratorIteratorFinish::NotFinished
      | GeneratorIteratorFinish::ExhaustedIterator => None,
      GeneratorIteratorFinish::GeneratorDone(r) => Some(r),
    }
  }
}

impl<Y, R, Q, I> Iterator for GeneratorIterator<Y, R, Q, I>
where
  I: Iterator<Item = Q>,
{
  type Item = Y;

  fn next(&mut self) -> Option<Self::Item> {
    // that's ergonomic
    if self.finished.finished() {
      return None;
    }

    let iteratee = match self.iter.next() {
      Some(it) => it,
      None => {
        self.finished = GeneratorIteratorFinish::ExhaustedIterator;
        return None;
      }
    };
    let gened = if matches!(
      self.generator.get_inner(),
      crate::wrapper::GeneratorWrapperInner::Unstarted(..)
    ) {
      self.generator.start(iteratee)
    } else {
      self.generator.query(iteratee)
    };
    match gened {
      GeneratorResponse::Yielding(y) => Some(y),
      GeneratorResponse::Done(r) => {
        self.finished = GeneratorIteratorFinish::GeneratorDone(r);
        None
      }
    }
  }
}

/// Tracks the state of the `GeneratorIterator`
pub enum GeneratorIteratorFinish<R> {
  /// We are still in normal operation
  NotFinished,
  /// The inner iterator ran out
  ExhaustedIterator,
  /// The outer generator ran out
  GeneratorDone(R),
}

impl<R> GeneratorIteratorFinish<R> {
  /// Returns `true` if the generator iterator finish is [`NotFinished`].
  ///
  /// [`NotFinished`]: GeneratorIteratorFinish::NotFinished
  #[must_use]
  pub fn finished(&self) -> bool {
    !matches!(self, Self::NotFinished)
  }
}
