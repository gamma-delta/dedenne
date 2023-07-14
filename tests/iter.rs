use dedenne::{iter::GeneratorIteratorFinish, *};

#[test]
fn exhaust_inner() {
  let iterator = 0u32..10;

  let mut geniterator = UnstartedGenerator::wrap(|y| async move {
    let mut acc = 1u32;
    // This loop can only run 10 times
    for i in 0..20 {
      acc *= 2;
      let got = y.ield(acc * 2).await;
      assert_eq!(got, i);
    }
    panic!("last i checked 20 > 10")
  })
  .iter_over(iterator);

  for x in &mut geniterator {
    assert!(x.is_power_of_two());
  }

  assert!(matches!(
    geniterator.finished,
    GeneratorIteratorFinish::ExhaustedIterator
  ));
}

#[test]
fn finish_generator() {
  // This iterator will only .next 10 elements
  let iterator =
    (0u32..20).chain(std::iter::from_fn(|| panic!("last i checked 20 > 10")));

  let mut geniterator = UnstartedGenerator::wrap(|y| async move {
    let mut acc = 1u32;
    for i in 0..10 {
      acc *= 2;
      let got = y.ield(acc).await;
      assert_eq!(got, i);
    }
    "All done!"
  })
  .iter_over(iterator);

  for x in &mut geniterator {
    assert!(x.is_power_of_two());
  }

  assert!(matches!(
    geniterator.finished,
    GeneratorIteratorFinish::GeneratorDone("All done!")
  ));
}
