use dedenne::{Generator, GeneratorResponse};

#[test]
fn wrapping() {
  let mut gen = Generator::new(|y, start| async move {
    let mut count = 0;
    for _ in 0..start {
      count += y.ield("One").await;
    }

    for _ in 0..count {
      y.ield("Two").await;
    }

    "All done!"
  });

  assert!(matches!(gen.start(5), GeneratorResponse::Yielding("One")));

  assert!(matches!(gen.query(3), GeneratorResponse::Yielding("One")));
  assert!(matches!(gen.query(7), GeneratorResponse::Yielding("One")));
  assert!(matches!(gen.query(1), GeneratorResponse::Yielding("One")));
  assert!(matches!(gen.query(2), GeneratorResponse::Yielding("One")));

  for _ in 0..(3 + 7 + 1 + 2) {
    assert!(matches!(gen.query(0), GeneratorResponse::Yielding("Two")));
  }

  assert!(matches!(gen.query(0), GeneratorResponse::Done("All done!")));
}
