//! Borrowed whole-string classification follows RFC 9193 Section 6 and
//! ADR 0029: Content-format declarations and recorded registry guidance.

pub(super) enum Declaration<'a> {
    Number(Option<u16>),
    Media {
        name: &'a str,
        codings: &'a str,
        parameters: &'a str,
    },
}

fn restricted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&byte)
}
fn tchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}
fn component(bytes: &[u8], index: &mut usize) -> Option<()> {
    let start = *index;
    if !bytes.get(start)?.is_ascii_alphanumeric() {
        return None;
    }
    while bytes.get(*index).is_some_and(|b| restricted(*b)) {
        *index += 1;
    }
    (*index - start <= 127).then_some(())
}
fn token(bytes: &[u8], index: &mut usize) -> Option<()> {
    let start = *index;
    while bytes.get(*index).is_some_and(|b| tchar(*b)) {
        *index += 1;
    }
    (*index != start).then_some(())
}
fn quoted(bytes: &[u8], index: &mut usize) -> Option<()> {
    *index += 1;
    loop {
        match *bytes.get(*index)? {
            b'"' => {
                *index += 1;
                return Some(());
            }
            b'\\' => {
                let escaped = *bytes.get(*index + 1)?;
                if !(0x20..=0x7e).contains(&escaped) {
                    return None;
                }
                *index += 2;
            }
            0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e => *index += 1,
            _ => return None,
        }
    }
}

/// Classify all authored text before applying the separate Section 3 range.
/// No AST, normalization, owned strings, parameter or transformation meaning.
pub(super) fn classify(text: &str) -> Option<Declaration<'_>> {
    let bytes = text.as_bytes();
    if !bytes.is_empty() && bytes.iter().all(u8::is_ascii_digit) {
        if bytes.len() > 1 && bytes[0] == b'0' {
            return None;
        }
        let value = if bytes.len() > 5 || bytes.len() == 5 && bytes > b"65535" {
            None
        } else {
            Some(
                bytes
                    .iter()
                    .fold(0u16, |value, byte| value * 10 + u16::from(byte - b'0')),
            )
        };
        return Some(Declaration::Number(value));
    }
    let mut index = 0;
    component(bytes, &mut index)?;
    if bytes.get(index) != Some(&b'/') {
        return None;
    }
    index += 1;
    component(bytes, &mut index)?;
    let name = &text[..index];
    let parameter_start = index;
    loop {
        let mut separator = index;
        while bytes.get(separator) == Some(&b' ') {
            separator += 1;
        }
        if bytes.get(separator) != Some(&b';') {
            break;
        }
        index = separator + 1;
        while bytes.get(index) == Some(&b' ') {
            index += 1;
        }
        token(bytes, &mut index)?;
        if bytes.get(index) != Some(&b'=') {
            return None;
        }
        index += 1;
        if bytes.get(index) == Some(&b'"') {
            quoted(bytes, &mut index)?;
        } else {
            token(bytes, &mut index)?;
        }
    }
    let coding_start = index;
    while bytes.get(index) == Some(&b'@') {
        index += 1;
        token(bytes, &mut index)?;
    }
    (index == bytes.len()).then_some(Declaration::Media {
        name,
        codings: &text[coding_start..],
        parameters: &text[parameter_start..coding_start],
    })
}
