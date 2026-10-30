use std::env;
use std::process::ExitCode;

use cli::run;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    run(env::args_os().collect()).await
}
