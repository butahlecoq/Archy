//! JSON transport for readers whose consumers may decode arbitrary byte chunks separately.
use serde::Serialize;
use std::fmt::Write as _;

fn escape_non_ascii(serialized: String) -> String {
    if serialized.is_ascii() {
        return serialized;
    }
    let mut ascii = String::with_capacity(serialized.len());
    for ch in serialized.chars() {
        if ch.is_ascii() {
            ascii.push(ch);
        } else {
            let mut units = [0_u16; 2];
            for unit in ch.encode_utf16(&mut units) {
                write!(&mut ascii, "\\u{unit:04x}").expect("writing to String cannot fail");
            }
        }
    }
    ascii
}

pub fn ascii_json_compact<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value).map(escape_non_ascii)
}

pub fn ascii_json_pretty<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value).map(escape_non_ascii)
}
