//! A thin client over ripx-core: parse arguments, call the engine, render the
//! graph. No analysis happens here.

mod options;
mod text;

use options::{Format, Options};

fn main() {
  if let Err(problem) = run() {
    eprintln!("sorry, {problem}");
    std::process::exit(1);
  }
}

fn run() -> Result<(), String> {
  let options = Options::from_args(std::env::args().skip(1))?;
  let graph = ripx_core::analyze(&options.manifest)?;

  // Diagnostics go to stderr so stdout stays a clean, pipeable report.
  for warning in graph.warnings() {
    eprintln!("warning: {warning}");
  }

  match options.format {
    Format::Json => println!("{}", graph.to_json()?),
    Format::Text => print!("{}", text::render(&graph)),
  }

  Ok(())
}
