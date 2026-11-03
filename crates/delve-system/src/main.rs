fn main() {
    if let Err(error) = angel_delve_system::serve_with(|run, inputs| run.step(inputs)) {
        eprintln!("Delve system stopped: {error}");
        std::process::exit(1);
    }
}
