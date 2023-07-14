use dedenne::*;

#[test]
fn non_unit_q() {
  let mut generator = Generator::wrap(|y| async move {
    let name = y.ield("What is your name?").await;
    let age = y.ield("How old are you?").await;
    let location = y.ield("Where are you from?").await;
    format!("Hello, {}, {} years old, from {}!", name, age, location)
  });

  assert!(matches!(
    generator.start(),
    GeneratorResponse::Yielding(y) if y == "What is your name?"
  ));
  assert!(matches!(
    generator.query("Petra"),
    GeneratorResponse::Yielding(y) if y == "How old are you?"
  ));
  assert!(matches!(
    generator.query("69"),
    GeneratorResponse::Yielding(y) if y == "Where are you from?"
  ));
  assert!(matches!(
    generator.query("Earth"),
    GeneratorResponse::Done(y)
      if y == "Hello, Petra, 69 years old, from Earth!"
  ));
}

#[test]
#[should_panic(expected = "Tried to query a generator after it had finished")]
fn safe_panic_after_stop() {
  let mut generator = Generator::wrap(|y| async move {
    y.ield(1).await;
    y.ield(2).await;
    "Finished!"
  });
  assert!(matches!(generator.start(), GeneratorResponse::Yielding(1)));
  assert!(matches!(generator.resume(), GeneratorResponse::Yielding(2)));

  assert!(matches!(
    generator.resume(),
    GeneratorResponse::Done("Finished!"),
  ));

  generator.resume();
  panic!("should not reach here")
}

#[test]
#[should_panic(expected = "Distinctive panic message!")]
fn panic_in_generator() {
  // this would be a Generator<i32, !, ()> if never type was stable
  let mut generator = Generator::wrap(|y| async move {
    y.ield(1).await;
    y.ield(2).await;
    panic!("Distinctive panic message!")
  });

  assert!(matches!(generator.start(), GeneratorResponse::Yielding(1)));
  assert!(matches!(generator.resume(), GeneratorResponse::Yielding(2)));
  generator.resume();
  panic!("should never get here!");
}