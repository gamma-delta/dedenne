# Dedenne

> Cute little generators!

Dedenne implements generators, a la the [unstable language feature](https://doc.rust-lang.org/stable/unstable-book/language-features/generators.html),
over async/await in completely stable Rust.

## Simple Usage

```rust
use dedenne::*;

fn example() {
  let mut generator = Generator::new(|y, init| async move {
    for x in 0..init {
      y.ield(x).await;
    }
    for x in (0..init).rev() {
      y.ield(x).await;
    }

    "All done!"
  });

  assert_eq!(
    generator.start(3), GeneratorResponse::Yielding(0)
  );
  assert_eq!(
    generator.resume(), GeneratorResponse::Yielding(1)
  );
  assert_eq!(
    generator.resume(), GeneratorResponse::Yielding(2)
  );
  assert_eq!(
    generator.resume(), GeneratorResponse::Yielding(1)
  );
  assert_eq!(
    generator.resume(), GeneratorResponse::Yielding(0)
  );
  assert_eq!(
    generator.resume(), GeneratorResponse::Done("All done!")
  );
}
```

## `panic!` vs `unreachable!`

If something in Dedenne `panic!`s, then it's a user error.
Make sure to `await` your `y.ield`s, and don't call `resume` after a 
generator's exhausted.

If something in Dedenne panics with an `unreachable!` message,
*then* it's a problem with Dedenne.
Please file a bug report if it does.
