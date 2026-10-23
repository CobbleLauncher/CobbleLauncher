// SPDX-License-Identifier: MIT
//! Bounded NBT visitor. Only names in the top-level servers list are retained.
use super::*;
struct Nbt<'a> {
    bytes: &'a [u8],
    at: usize,
    nodes: usize,
    endpoints: Vec<String>,
    include_addresses: bool,
}
impl<'a> Nbt<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("Invalid server data")?;
        let result = self.bytes.get(self.at..end).ok_or("Invalid server data")?;
        self.at = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.take(2)?;
        let size = u16::from_be_bytes([n[0], n[1]]) as usize;
        String::from_utf8(self.take(size)?.to_vec()).map_err(|_| "Invalid server name".into())
    }
    fn length(&mut self) -> Result<usize, String> {
        let n = self.take(4)?;
        let len = i32::from_be_bytes(n.try_into().map_err(|_| "Invalid server data")?);
        if !(0..=1_048_576).contains(&len) {
            return Err("Invalid server data".into());
        }
        Ok(len as usize)
    }
    fn value(
        &mut self,
        tag: u8,
        depth: usize,
        server: bool,
        names: &mut Vec<Value>,
    ) -> Result<(), String> {
        self.nodes += 1;
        if depth > 16 || self.nodes > 10_000 {
            return Err("Server data limit exceeded".into());
        }
        match tag {
            1 => {
                self.take(1)?;
            }
            2 => {
                self.take(2)?;
            }
            3 | 5 => {
                self.take(4)?;
            }
            4 | 6 => {
                self.take(8)?;
            }
            7 | 11 | 12 => {
                let n = self.length()?;
                let width = if tag == 11 {
                    4
                } else if tag == 12 {
                    8
                } else {
                    1
                };
                self.take(n.checked_mul(width).ok_or("Invalid server data")?)?;
            }
            8 => {
                self.string()?;
            }
            9 => {
                let child = self.byte()?;
                let n = self.length()?;
                if n > LIMIT {
                    return Err("Server entry limit exceeded".into());
                }
                for _ in 0..n {
                    self.value(child, depth + 1, server, names)?;
                }
            }
            10 => {
                let mut name = String::new();
                let mut address = String::new();
                loop {
                    let child = self.byte()?;
                    if child == 0 {
                        break;
                    }
                    let key = self.string()?;
                    if server && child == 8 && matches!(key.as_str(), "name" | "ip") {
                        let value = self.string()?;
                        if key == "name" {
                            name = value;
                        } else {
                            address = value;
                        }
                    } else {
                        self.value(child, depth + 1, depth == 0 && key == "servers", names)?;
                    }
                }
                if server {
                    let canonical = address.trim().to_ascii_lowercase();
                    let duplicate = self
                        .endpoints
                        .iter()
                        .position(|value| !canonical.is_empty() && value == &canonical);
                    let index = names.len();
                    names.push(json!({"id":index,"name":clean(&name),"duplicateOf":duplicate,"addressKind":address_kind(&canonical)}));
                    if self.include_addresses {
                        names.last_mut().ok_or("Invalid server data")?["address"] =
                            json!(address.trim());
                    }
                    self.endpoints.push(canonical);
                }
            }
            _ => return Err("Unsupported server data".into()),
        }
        Ok(())
    }
}
fn address_kind(value: &str) -> &'static str {
    let host = if value.starts_with('[') {
        value
            .split(']')
            .next()
            .unwrap_or("")
            .trim_start_matches('[')
    } else {
        value.split(':').next().unwrap_or("")
    };
    if host == "localhost" {
        return "local";
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip))
            if ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() =>
        {
            "local"
        }
        Ok(std::net::IpAddr::V6(ip))
            if ip.is_loopback()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_unspecified() =>
        {
            "local"
        }
        Ok(_) => "public",
        Err(_) => "unknown",
    }
}
fn parse(bytes: &[u8]) -> Result<Vec<Value>, String> {
    parse_mode(bytes, false)
}
fn parse_mode(bytes: &[u8], include_addresses: bool) -> Result<Vec<Value>, String> {
    let mut nbt = Nbt {
        bytes,
        at: 0,
        nodes: 0,
        endpoints: vec![],
        include_addresses,
    };
    if nbt.byte()? != 10 {
        return Err("Invalid server data".into());
    }
    nbt.string()?;
    let mut names = vec![];
    nbt.value(10, 0, false, &mut names)?;
    Ok(names)
}
pub(super) fn collect(root: &Path) -> Result<Value, String> {
    collect_mode(root, false)
}
pub(in crate::games) fn saved(root: &Path) -> Result<Value, String> {
    collect_mode(root, true)
}
fn collect_mode(root: &Path, include_addresses: bool) -> Result<Value, String> {
    let path = root.join("servers.dat");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({"items":[],"available":true,"truncated":false}));
        }
        Err(_) => return Err("Server list unavailable".into()),
    };
    if linked(&metadata) || !metadata.is_file() || metadata.len() > BYTE_LIMIT {
        return Err("Server list exceeds safe limits".into());
    }
    let mut bytes = vec![];
    fs::File::open(path)
        .map_err(|_| "Server list unavailable")?
        .take(BYTE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Server list unavailable")?;
    if bytes.len() as u64 > BYTE_LIMIT {
        return Err("Server list exceeds safe limits".into());
    }
    Ok(
        json!({"items":if include_addresses {parse_mode(&bytes, true)?} else {parse(&bytes)?},"available":true,"truncated":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_only_and_malformed_data_rejected() {
        let bytes=b"\x0a\x00\x00\x09\x00\x07servers\x0a\x00\x00\x00\x01\x08\x00\x04name\x00\x04Home\x08\x00\x02ip\x00\x09secret:42\x00\x00";
        let result = parse(bytes).unwrap();
        assert_eq!(result[0]["name"], "Home");
        assert_eq!(result[0]["addressKind"], "unknown");
        assert!(parse(&bytes[..12]).is_err());
        assert!(!serde_json::to_string(&result).unwrap().contains("secret"));
        assert_eq!(parse_mode(bytes, true).unwrap()[0]["address"], "secret:42");
    }
    #[test]
    fn syntax_classification_does_not_resolve_domains() {
        assert_eq!(address_kind("127.0.0.1:25565"), "local");
        assert_eq!(address_kind("8.8.8.8:25565"), "public");
        assert_eq!(address_kind("example.org"), "unknown");
    }
}
