use core::time::Duration;
use std::io::{self, BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{JoinHandle, sleep, spawn};

const DONE: &str = "DONE";
const PAUSE: char = '\u{0}';
const STEP: Duration = Duration::from_millis(150);
const TAG: &str = "<tag>";

pub(crate) struct Fake {
    port: u16,
    seen: Receiver<String>,
    worker: JoinHandle<()>,
}

impl Fake {
    pub(crate) fn finish(self) -> Vec<String> {
        let found = self.seen.try_iter().collect();
        drop(self.worker.join());
        found
    }

    pub(crate) const fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn url(&self) -> String {
        format!("imap://127.0.0.1:{}", self.port)
    }
}

pub(crate) type Script = Vec<Step>;

pub(crate) type Step = (&'static str, String);

fn converse(stream: TcpStream, script: &[Step], sender: &Sender<String>) -> io::Result<()> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let Some((greeting, steps)) = script.split_first() else {
        return Ok(());
    };
    speak(&mut writer, &greeting.1, "")?;
    let mut last = String::new();
    for step in steps {
        let mut raw = String::new();
        if reader.read_line(&mut raw)? == 0 {
            return Ok(());
        }
        let line = raw.trim_end();
        let (found, rest) = line.split_once(' ').unwrap_or(("", line));
        let (tag, command) = if line == DONE {
            (last.clone(), line.to_owned())
        } else {
            (found.to_owned(), rest.to_owned())
        };
        drop(sender.send(command.clone()));
        if !command.starts_with(step.0) {
            return Ok(());
        }
        speak(&mut writer, &step.1, &tag)?;
        last = tag;
    }
    Ok(())
}

pub(crate) fn fetched(uid: u64, raw: &str) -> String {
    format!(
        "* {uid} FETCH (UID {uid} INTERNALDATE \"08-Jul-2026 12:00:35 +0800\" BODY[]<0> {{{}}}\r\n{raw})\r\n",
        raw.len()
    )
}

pub(crate) fn hello(capabilities: &str) -> Script {
    vec![
        ("", "* OK ready\r\n".to_owned()),
        (
            "CAPABILITY",
            format!("* CAPABILITY IMAP4rev1 {capabilities}\r\n<tag> OK done\r\n"),
        ),
        ("LOGIN", "<tag> OK logged in\r\n".to_owned()),
        (
            "SELECT",
            "* OK [UIDVALIDITY 3] ok\r\n* OK [UIDNEXT 8] ok\r\n<tag> OK [READ-WRITE] selected\r\n"
                .to_owned(),
        ),
    ]
}

pub(crate) fn serve(sessions: Vec<Script>) -> io::Result<Fake> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let (sender, seen) = channel();
    let count = sessions.len();
    let worker = spawn(move || {
        for (stream, script) in listener.incoming().take(count).flatten().zip(sessions) {
            drop(converse(stream, &script, &sender));
        }
    });
    Ok(Fake { port, seen, worker })
}

fn speak(writer: &mut TcpStream, reply: &str, tag: &str) -> io::Result<()> {
    for (index, part) in reply.split(PAUSE).enumerate() {
        if index > 0 {
            sleep(STEP);
        }
        writer.write_all(part.replace(TAG, tag).as_bytes())?;
    }
    Ok(())
}
