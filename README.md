# Dedenne

> Cute little generators!

Dedenne implements generators, a la the [unstable language feature](https://doc.rust-lang.org/stable/unstable-book/language-features/generators.html),
over async/await in completely stable Rust.

## Simple Usage

```rust
use dedenne::*;

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
```

For a larger example, [check out this simple TUI interface](https://github.com/gamma-delta/dedenne/blob/main/examples/ui.rs).

## `panic!` vs `unreachable!`

If something in Dedenne `panic!`s, then it's a user error.
Make sure to `await` your `y.ield`s, and don't call `resume` after a 
generator's exhausted.

If something in Dedenne panics with an `unreachable!` message,
*then* it's a problem with Dedenne.
Please file a bug report if it does.

## Prior Art

I am not the first person to have this idea.
However, I think Dedenne is the only crate that supports mapping over iterators.

- [`generator`](https://crates.io/crates/generator).
  Doesn't support a starting argument,
  which means you can't use it as a mapping iterator.
  Is always stackful.
- [`genawaiter`](https://crates.io/crates/genawaiter).
  Has some convenience macros that make it nicer to make generators.
  Also lets you really tune how the generators are stored
  (stackful or allocating).
- [Unstable lang feature](https://github.com/rust-lang/rfcs/pull/2033).
  Doesn't support passing values *in* to a generator, afaict.
  Also requires nightly rust and has been bikeshedding for 6 years
  as of time of writing.
