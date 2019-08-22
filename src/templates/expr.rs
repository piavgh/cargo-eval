#{prelude}

fn main() {
  let expr = || -> Result<(), Box<dyn std::error::Error>> {
    println!("{:?}", {#{script}});
    Ok(())
  };

  if let Err(e) = expr() {
    eprintln!("Error: {}", e);
    std::process::exit(1);
  }
}
