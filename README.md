# Dedenne

> Cute little generators!

Dedenne implements generators, a la the [unstable language feature](https://doc.rust-lang.org/stable/unstable-book/language-features/generators.html),
over async/await in completely stable Rust.

## Simple Usage

```rust
use dedenne::*;

fn example() {
  // A generator that returns only numbers divisible by 7.
  let generator = UnstartedGenerator::wrap(|y| async move {
    for x in 2u32..1_000_000 {
      if x % 7 == 0 {
        y.ield(x);
      }
    }
    "All done!"
  });

  for yielded in generator.iter() {
    assert!(yielded % 7 == 0);
  }
}
```

## `panic!` vs `unreachable!`

If something in Dedenne `panic!`s, then it's a user error.
Make sure to `await` your `y.ield`s, and don't call `resume` after a 
generator's exhausted.

If something in Dedenne panics with an `unreachable!` message,
*then* it's a problem with Dedenne.
Please file a bug report if it does.
