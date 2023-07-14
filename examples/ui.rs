//! This example could be done with blocking calls to read_line,
//! but in a more complicated application (like a game where you
//! want animations to continue in the background), you need
//! generators/coroutines.

use std::io::{self, Write};

use dedenne::{GeneratorResponse, StartedGenerator, YieldWrapper};

struct BazQuxxInstaller {
  y: YieldWrapper<String, String>,
}

impl BazQuxxInstaller {
  pub async fn start(y: YieldWrapper<String, String>) {
    let me = Self::new(y);
    me.root().await
  }

  fn new(y: YieldWrapper<String, String>) -> Self {
    Self { y }
  }

  async fn root(&self) {
    loop {
      let action = self
        .read(
          "Welcome to the BazQuxx Installer!\n\
          Pick `install`, `uninstall`, or `quit`:",
        )
        .await;
      match action.as_str() {
        "quit" => break,
        "install" => self.install().await,
        "uninstall" => self.uninstall().await,
        ono => {
          println!("I don't know how to {}", ono);
        }
      }
    }
  }

  async fn install(&self) {
    let path = self
      .read(
        "Where would you like to install? \
        (Type anything you want, this program doesn't install anything)",
      )
      .await;
    let do_baz = self.read_bool("Do you want the baz feature?").await;
    let do_quxx = self.read_bool("Do you want the quxx feature?").await;
    println!(
      "Installing BazQuxx with baz: {} and quxx: {} at {:?}",
      do_baz, do_quxx, path
    );
  }

  async fn uninstall(&self) {
    let enjoyed = self
      .read_bool("Did you enjoy your experience with BazQuxx?")
      .await;
    if enjoyed {
      println!("Glad to hear it!");
    } else {
      let count = self
        .read(
          "Oh ok buddy just for that \
          (and to demonstrate control flow) \
          I'm making you sit through more.",
        )
        .await
        .len();
      let count = count + self.read("Yep, still going.").await.len();
      let count = count + self.read("I can do this all day. ").await.len();
      let count = count
        + self
          .read("I'm a computer, I literally don't get bored.")
          .await
          .len();
      println!(
        "Ok, fine. In case you care,\
        you keysmashed {} characters during all that.",
        count
      );
    }
  }

  async fn read<S: AsRef<str>>(&self, msg: S) -> String {
    let msg = msg.as_ref();
    self.y.ield(format!("{} ", msg)).await
  }

  async fn read_bool<S: AsRef<str>>(&self, msg: S) -> bool {
    let msg = msg.as_ref();
    loop {
      let resp = self.read(format!("{} [yN]", msg)).await;
      let resp_lower = resp.to_lowercase();
      let cleaned = resp_lower.as_str();

      if cleaned == "y" || cleaned == "yes" {
        return true;
      } else if cleaned == "" || cleaned == "n" || cleaned == "no" {
        return false;
      } else {
        println!("Didn't recognize {}", resp);
        // loop back to top
      }
    }
  }
}

fn main() -> io::Result<()> {
  let (mut generator, mut output) = StartedGenerator::start(|y| async move {
    BazQuxxInstaller::start(y).await;
  });
  let mut stdout = io::stdout();
  loop {
    let msg = match output {
      GeneratorResponse::Yielding(msg) => msg,
      GeneratorResponse::Done(()) => break,
    };
    write!(&mut stdout, "{}", msg)?;
    stdout.flush()?;

    let response = readline()?;
    output = generator.query(response);
  }

  Ok(())
}

/// Pretend this absolutely cannot be blocking and is instead
/// implemented by accumulating character events or something.
fn readline() -> io::Result<String> {
  let mut buf = String::new();
  let stdin = io::stdin();
  Ok(if stdin.read_line(&mut buf)? == 0 {
    buf
  } else {
    // Remove pesky newline
    buf.pop();
    buf
  })
}
