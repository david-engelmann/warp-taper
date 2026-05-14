use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "warp-taper",
    version,
    about = "Record Warp behavior into PR-ready evidence bundles."
)]
struct Cli {
    /// Subcommand to run. The full command surface lands in P5.
    #[arg(default_value = "version")]
    command: String,
}

fn main() {
    let cli = Cli::parse();
    match cli.command.as_str() {
        "version" => println!("warp-taper {}", env!("CARGO_PKG_VERSION")),
        other => {
            eprintln!("warp-taper: unimplemented subcommand '{other}' (CLI surface lands in P5)");
            std::process::exit(2);
        }
    }
}
