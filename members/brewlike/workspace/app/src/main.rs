use brewlike::cli;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = cli::app()?;
    app.verify_command(&cli::command())?;
    app.run(cli::command(), std::env::args());
    Ok(())
}
