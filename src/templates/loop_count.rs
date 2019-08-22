use std::any::Any;
use std::io::BufRead;

fn main() {
  let mut closure = assert_closure({
    #{script}
  });

  let stdin = std::io::stdin();
  let mut it = stdin.lock().lines().enumerate();

  while let Some((i, Ok(line))) = it.next()  {
    let output = closure(line, i);

    let display = {
      let output_any: &dyn Any = &output;
      !output_any.is::<()>()
    };

    if display {
      println!("{:?}", output);
    }
  }
}

fn assert_closure<F, T>(closure: F) -> F
  where
    F: FnMut(String, usize) -> T
{
  closure
}
