const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,";
const MASK: u32 = 63;
const SEXTET: u32 = 6;
const UNIT: u32 = 16;

pub(super) fn encode(name: &str) -> String {
    let mut out = String::new();
    let mut run: Vec<u16> = Vec::new();
    for letter in name.chars() {
        if (' '..='~').contains(&letter) {
            flush(&mut out, &mut run);
            out.push(letter);
            if letter == '&' {
                out.push('-');
            }
        } else {
            let mut units = [0; 2];
            run.extend_from_slice(letter.encode_utf16(&mut units));
        }
    }
    flush(&mut out, &mut run);
    out
}

fn flush(out: &mut String, run: &mut Vec<u16>) {
    if run.is_empty() {
        return;
    }
    out.push('&');
    let mut bits: u32 = 0;
    let mut count: u32 = 0;
    for unit in run.drain(..) {
        bits = bits.wrapping_shl(UNIT) | u32::from(unit);
        count = count.saturating_add(UNIT);
        while count >= SEXTET {
            count = count.saturating_sub(SEXTET);
            out.push(sextet(bits.wrapping_shr(count)));
        }
    }
    if count > 0 {
        out.push(sextet(bits.wrapping_shl(SEXTET.saturating_sub(count))));
    }
    out.push('-');
}

fn sextet(value: u32) -> char {
    let index = usize::try_from(value & MASK).unwrap_or_default();
    ALPHABET.get(index).copied().map_or('A', char::from)
}
