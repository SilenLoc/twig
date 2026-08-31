//! Minimal pkt-line codec, the framing git uses to talk to `proc-receive`.
//!
//! A packet is a four-character hex length covering the header itself, followed
//! by the payload. `0000` is a flush packet and terminates a section.

use std::io::{Read, Write};

/// Reads one packet. Returns `None` on a flush packet or at end of input.
pub fn read(reader: &mut impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut header = [0u8; 4];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = std::str::from_utf8(&header)
        .ok()
        .and_then(|s| usize::from_str_radix(s, 16).ok())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid pkt-line header: {header:?}"),
            )
        })?;

    // 0 is flush; 1 and 2 are delimiter/response-end and carry no payload.
    if len <= 4 {
        return Ok(None);
    }

    let mut payload = vec![0u8; len - 4];
    reader.read_exact(&mut payload)?;
    Ok(Some(payload))
}

/// Reads packets until a flush.
pub fn read_section(reader: &mut impl Read) -> std::io::Result<Vec<String>> {
    let mut lines = Vec::new();
    while let Some(packet) = read(reader)? {
        lines.push(String::from_utf8_lossy(&packet).trim_end().to_string());
    }
    Ok(lines)
}

pub fn write(writer: &mut impl Write, line: &str) -> std::io::Result<()> {
    let len = line.len() + 4;
    if len > 0xFFFF {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "pkt-line payload too large",
        ));
    }
    write!(writer, "{len:04x}{line}")
}

pub fn flush(writer: &mut impl Write) -> std::io::Result<()> {
    writer.write_all(b"0000")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_frames_length_including_header() {
        let mut out = Vec::new();
        write(&mut out, "version=1\n").unwrap();
        assert_eq!(out, b"000eversion=1\n");
    }

    #[test]
    fn test_flush_is_four_zeros() {
        let mut out = Vec::new();
        flush(&mut out).unwrap();
        assert_eq!(out, b"0000");
    }

    #[test]
    fn test_read_roundtrip() {
        let mut out = Vec::new();
        write(&mut out, "hello\n").unwrap();
        flush(&mut out).unwrap();

        let mut cursor = std::io::Cursor::new(out);
        assert_eq!(read(&mut cursor).unwrap(), Some(b"hello\n".to_vec()));
        assert_eq!(read(&mut cursor).unwrap(), None, "flush ends the section");
    }

    #[test]
    fn test_read_section_collects_until_flush() {
        let mut out = Vec::new();
        write(&mut out, "one\n").unwrap();
        write(&mut out, "two\n").unwrap();
        flush(&mut out).unwrap();
        write(&mut out, "after\n").unwrap();

        let mut cursor = std::io::Cursor::new(out);
        let section = read_section(&mut cursor).unwrap();
        assert_eq!(section, vec!["one", "two"]);

        // The reader is positioned at the next section, not consumed past it.
        assert_eq!(read(&mut cursor).unwrap(), Some(b"after\n".to_vec()));
    }

    #[test]
    fn test_read_at_eof_is_none() {
        let mut cursor = std::io::Cursor::new(Vec::new());
        assert_eq!(read(&mut cursor).unwrap(), None);
    }

    #[test]
    fn test_invalid_header_is_an_error() {
        let mut cursor = std::io::Cursor::new(b"zzzzpayload".to_vec());
        assert!(read(&mut cursor).is_err());
    }
}
