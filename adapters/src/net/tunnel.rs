use super::{Wire, code, pull, push};

const BYTE: u16 = 8;
const HEAD_MAX: usize = 0x2000;
const LOW: u16 = 0xff;
const NO_AUTH: [u8; 3] = [5, 1, 0];
const VERSION: u8 = 5;

pub(super) async fn connect(wire: &mut Wire, host: &str, port: u16) -> Result<(), String> {
    let authority = format!("{host}:{port}");
    let opening = format!("CONNECT {authority} HTTP/1.1\r\nhost: {authority}\r\n\r\n");
    let sent = push(wire, opening.as_bytes()).await;
    let head = sent.and(head(wire).await)?;
    let text = String::from_utf8_lossy(&head);
    let status = text
        .lines()
        .next()
        .and_then(code)
        .ok_or_else(|| "io".to_owned())?;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("proxy:{status}"))
    }
}

async fn fill(wire: &mut Wire, buffer: &mut [u8]) -> Result<(), String> {
    let mut done = 0;
    while let Some(rest) = buffer.get_mut(done..).filter(|rest| !rest.is_empty()) {
        let size = pull(wire, rest).await?;
        if size == 0 {
            return Err("io".to_owned());
        }
        done = done.saturating_add(size);
    }
    Ok(())
}

async fn head(wire: &mut Wire) -> Result<Vec<u8>, String> {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [u8::default(); 1];
        let size = pull(wire, &mut byte).await?;
        if size == 0 || head.len() >= HEAD_MAX {
            return Err("io".to_owned());
        }
        head.extend_from_slice(&byte);
    }
    Ok(head)
}

pub(super) async fn socks(wire: &mut Wire, host: &str, port: u16) -> Result<(), String> {
    let length = u8::try_from(host.len()).map_err(|_err| "proxy:host".to_owned())?;
    let mut chosen = [u8::default(); 2];
    let greeted = push(wire, &NO_AUTH).await;
    greeted.and(fill(wire, &mut chosen).await)?;
    if chosen != [VERSION, 0] {
        return Err("proxy:method".to_owned());
    }
    let mut request = vec![VERSION, 1, 0, 3, length];
    request.extend_from_slice(host.as_bytes());
    request.push(u8::try_from(port >> BYTE).unwrap_or_default());
    request.push(u8::try_from(port & LOW).unwrap_or_default());
    let mut reply = [u8::default(); 4];
    let asked = push(wire, &request).await;
    asked.and(fill(wire, &mut reply).await)?;
    let [version, status, _, kind] = reply;
    if version != VERSION {
        return Err("io".to_owned());
    }
    if status != 0 {
        return Err(format!("proxy:{status}"));
    }
    let size = match kind {
        1 => 4,
        4 => 16,
        3 => {
            let mut named = [u8::default(); 1];
            fill(wire, &mut named).await?;
            let [count] = named;
            usize::from(count)
        }
        _ => return Err("io".to_owned()),
    };
    let mut bound = vec![u8::default(); size.saturating_add(2)];
    fill(wire, &mut bound).await
}
