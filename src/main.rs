use clap::Parser as _;
use transcripter::cli::Cli;

fn main() {
    #[cfg(feature = "gui-core")]
    if std::env::args_os().len() == 1 {
        transcripter::gui::run();
        return;
    }

    if let Err(error) = transcripter::run_cli(Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
