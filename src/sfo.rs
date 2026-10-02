//! PS4 game metadata: `sce_sys/param.sfo`, a small key/value file ("PSF"). Every offset is
//! bounds-checked, so a damaged file yields nothing rather than garbage.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Int(u32),
}

impl Value {
    pub fn text(&self) -> &str {
        match self {
            Value::Text(s) => s,
            Value::Int(_) => "",
        }
    }
}

/// Keys and values of a param.sfo file.
pub fn parse(bytes: &[u8]) -> Option<HashMap<String, Value>> {
    let u16_at = |o: usize| bytes.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    if bytes.get(..4)? != b"\0PSF" {
        return None;
    }
    let (keys, data, count) = (u32_at(8)? as usize, u32_at(12)? as usize, u32_at(16)? as usize);
    if count > 1024 {
        return None;
    }
    let mut out = HashMap::new();
    for i in 0..count {
        let e = 20 + i * 16;
        let (key_off, format, len, data_off) = (u16_at(e)? as usize, u16_at(e + 2)?, u32_at(e + 4)? as usize, u32_at(e + 12)? as usize);
        let key_start = keys.checked_add(key_off)?;
        let key_end = key_start + bytes.get(key_start..)?.iter().position(|b| *b == 0)?;
        let key = std::str::from_utf8(&bytes[key_start..key_end]).ok()?.to_string();
        let start = data.checked_add(data_off)?;
        let raw = bytes.get(start..start.checked_add(len)?)?;
        let value = match format {
            0x0404 => Value::Int(u32::from_le_bytes(raw.get(..4)?.try_into().ok()?)),
            // UTF-8 text, NUL-terminated (0x0204) or not (0x0004).
            _ => Value::Text(String::from_utf8_lossy(raw).trim_end_matches('\0').to_string()),
        };
        out.insert(key, value);
    }
    Some(out)
}

/// Builds param.sfo files for tests.
#[cfg(test)]
pub fn build(entries: &[(&str, Value)]) -> Vec<u8> {
    let mut keys = Vec::new();
    let mut data = Vec::new();
    let mut index = Vec::new();
    for (k, v) in entries {
        let key_off = keys.len() as u16;
        keys.extend_from_slice(k.as_bytes());
        keys.push(0);
        let (format, raw) = match v {
            Value::Text(s) => (0x0204u16, [s.as_bytes(), b"\0"].concat()),
            Value::Int(n) => (0x0404u16, n.to_le_bytes().to_vec()),
        };
        let data_off = data.len() as u32;
        index.extend_from_slice(&key_off.to_le_bytes());
        index.extend_from_slice(&format.to_le_bytes());
        index.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        index.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        index.extend_from_slice(&data_off.to_le_bytes());
        data.extend(raw);
    }
    while keys.len() % 4 != 0 {
        keys.push(0);
    }
    let keys_start = 20 + index.len() as u32;
    let mut out = b"\0PSF".to_vec();
    out.extend_from_slice(&0x0101u32.to_le_bytes());
    out.extend_from_slice(&keys_start.to_le_bytes());
    out.extend_from_slice(&(keys_start + keys.len() as u32).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    out.extend(index);
    out.extend(keys);
    out.extend(data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_and_damage() {
        let bytes = build(&[("TITLE", Value::Text("Firewatch".into())), ("TITLE_ID", Value::Text("CUSA04118".into())), ("APP_TYPE", Value::Int(1))]);
        let p = parse(&bytes).unwrap();
        assert_eq!(p["TITLE"].text(), "Firewatch");
        assert_eq!(p["TITLE_ID"].text(), "CUSA04118");
        assert_eq!(p["APP_TYPE"], Value::Int(1));
        assert!(parse(b"nope").is_none());
        assert!(parse(&bytes[..30]).is_none(), "truncated files are rejected");
    }
}
