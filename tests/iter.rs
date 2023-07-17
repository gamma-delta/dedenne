use dedenne::*;

#[test]
fn smoke_test() {
  let (mut gen, resp) = StartedGenerator::run(|y| async move {
    y.ield(-5).await;
    y.ield(-7).await;
    // and from now on, only positives
    for x in 1..100 {
      y.ield(x).await;
    }
    "All done!"
  });
  assert!(matches!(resp, GeneratorResponse::Yielding(-5)));
  assert!(matches!(gen.resume(), GeneratorResponse::Yielding(-7)));

  let mut iter = gen.iter();
  for yielded in &mut iter {
    assert!(yielded > 0);
  }

  assert_eq!(iter.consume_response(), Some("All done!"));
}
