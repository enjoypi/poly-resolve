use std::process::{Command, ExitCode};

use crate::support::run;

#[tokio::test]
async fn call_subcommand_is_gone() {
    assert_eq!(run(&["cli", "call"]).await, ExitCode::from(2));
    let help = Command::new(env!("CARGO_BIN_EXE_cli"))
        .arg("--help")
        .output()
        .unwrap();
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("serve") && !text.contains("call"));
}

#[tokio::test]
async fn help_and_bad_args_follow_clap_codes() {
    assert_eq!(run(&["cli", "--help"]).await, ExitCode::SUCCESS);
    assert_eq!(run(&["cli", "bogus"]).await, ExitCode::from(2));
    assert_eq!(run(&["cli"]).await, ExitCode::from(2));
}
