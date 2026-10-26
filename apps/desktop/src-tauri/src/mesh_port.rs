// SPDX-License-Identifier: MIT
//! Read-only local listener ownership check; renderer never provides a command or target host.
pub(super) fn approved_server_listener(port: u16) -> Result<(), String> {
    if port < 1024 {
        return Err("请输入 1024–65535 的服务器端口".into());
    }
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_secs(1),
    )
    .map(|_| ())
    .map_err(|_| "本机服务器端口尚未开放".into())
}
pub(super) fn approved_listener(port: u16, pid: u32) -> Result<(), String> {
    validate_listener(port, pid, true)
}
pub(super) fn owned_listener(port: u16, pid: u32) -> Result<(), String> {
    validate_listener(port, pid, false)
}
fn validate_listener(port: u16, pid: u32, probe: bool) -> Result<(), String> {
    if port < 1024 {
        return Err("请输入 1024–65535 的游戏局域网端口".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let root = std::env::var_os("SystemRoot").ok_or("无法读取系统网络工具位置")?;
        use std::io::Read;
        let child =
            std::process::Command::new(std::path::PathBuf::from(root).join("System32/netstat.exe"))
                .args(["-ano", "-p", "TCP"])
                .creation_flags(0x0800_0000)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|_| "无法验证游戏端口所属进程")?;
        struct OwnedQuery(std::process::Child);
        impl Drop for OwnedQuery {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = OwnedQuery(child);
        let output = child.0.stdout.take().ok_or("无法验证游戏端口所属进程")?;
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            output
                .take(2 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        let started = std::time::Instant::now();
        let status = loop {
            if let Some(status) = child.0.try_wait().map_err(|_| "无法验证游戏端口所属进程")?
            {
                break status;
            }
            if started.elapsed() >= std::time::Duration::from_secs(3) {
                return Err("游戏端口验证超时".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let output = reader
            .join()
            .map_err(|_| "无法验证游戏端口所属进程")?
            .map_err(|_| "无法验证游戏端口所属进程")?;
        if !status.success() || output.len() > 2 * 1024 * 1024 {
            return Err("无法验证游戏端口所属进程".into());
        }
        let text = String::from_utf8_lossy(&output);
        if !listener_owned(&text, port, pid) {
            return Err("该端口不属于选定的游戏，请先在游戏中对局域网开放".into());
        }
        if probe {
            std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
                std::time::Duration::from_secs(1),
            )
            .map_err(|_| "游戏局域网端口尚未开放")?;
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        linux_listener_owned(port, pid)?;
        if probe {
            std::net::TcpStream::connect_timeout(
                &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
                std::time::Duration::from_secs(1),
            )
            .map_err(|_| "游戏局域网端口尚未开放")?;
        }
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (pid, probe);
        Err("此平台尚不支持验证游戏端口所属进程".into())
    }
}

#[cfg(target_os = "linux")]
fn linux_listener_owned(port: u16, pid: u32) -> Result<(), String> {
    use std::collections::HashSet;
    use std::io::Read;

    const MAX_PROC_BYTES: u64 = 2 * 1024 * 1024;
    const MAX_FDS: usize = 16_384;
    const OWNERSHIP_ERROR: &str = "无法验证游戏端口所属进程";
    const NOT_OWNED: &str = "该端口不属于选定的游戏，请先在游戏中对局域网开放";

    // /proc/<tid> also exists for a thread and shares its process's fd table.
    // Only an actual process ID may satisfy the selected game PID check.
    let status = std::fs::File::open(format!("/proc/{pid}/status")).map_err(|_| OWNERSHIP_ERROR)?;
    let mut status_bytes = Vec::new();
    status
        .take(64 * 1024 + 1)
        .read_to_end(&mut status_bytes)
        .map_err(|_| OWNERSHIP_ERROR)?;
    if status_bytes.len() > 64 * 1024
        || !std::str::from_utf8(&status_bytes)
            .map_err(|_| OWNERSHIP_ERROR)?
            .lines()
            .any(|line| {
                line.strip_prefix("Tgid:")
                    .is_some_and(|value| value.trim().parse::<u32>().ok() == Some(pid))
            })
    {
        return Err(OWNERSHIP_ERROR.into());
    }

    let mut inodes = HashSet::new();
    for table in ["tcp", "tcp6"] {
        // Read the game's network namespace, rather than the launcher's namespace.
        let path = format!("/proc/{pid}/net/{table}");
        let file = std::fs::File::open(path).map_err(|_| OWNERSHIP_ERROR)?;
        let mut bytes = Vec::new();
        file.take(MAX_PROC_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| OWNERSHIP_ERROR)?;
        if bytes.len() as u64 > MAX_PROC_BYTES {
            return Err(OWNERSHIP_ERROR.into());
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| OWNERSHIP_ERROR)?;
        inodes.extend(linux_listening_inodes(text, port));
    }
    if inodes.is_empty() {
        return Err(NOT_OWNED.into());
    }

    let entries = std::fs::read_dir(format!("/proc/{pid}/fd")).map_err(|_| OWNERSHIP_ERROR)?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_FDS {
            return Err(OWNERSHIP_ERROR.into());
        }
        let entry = entry.map_err(|_| OWNERSHIP_ERROR)?;
        let Ok(target) = std::fs::read_link(entry.path()) else {
            // A descriptor can close while /proc is being enumerated.
            continue;
        };
        let Some(target) = target.to_str() else {
            continue;
        };
        let Some(inode) = target
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        if inodes.contains(&inode) {
            return Ok(());
        }
    }
    Err(NOT_OWNED.into())
}

#[cfg(target_os = "linux")]
fn linux_listening_inodes(text: &str, port: u16) -> impl Iterator<Item = u64> + '_ {
    text.lines().filter_map(move |line| {
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() < 10 || columns[3] != "0A" {
            return None;
        }
        let (address, hex_port) = columns[1].split_once(':')?;
        if u16::from_str_radix(hex_port, 16).ok()? != port {
            return None;
        }
        // /proc/net/tcp{,6} prints each 32-bit address word little-endian.
        // Match only wildcard and loopback listeners, as on Windows.
        let local = match address.len() {
            8 => {
                address.eq_ignore_ascii_case("00000000") || address.eq_ignore_ascii_case("0100007F")
            }
            32 => {
                address.eq_ignore_ascii_case("00000000000000000000000000000000")
                    || address.eq_ignore_ascii_case("00000000000000000000000001000000")
            }
            _ => false,
        };
        local.then(|| columns[9].parse::<u64>().ok()).flatten()
    })
}

#[cfg(any(windows, test))]
fn listener_owned(text: &str, port: u16, pid: u32) -> bool {
    text.lines().any(|line| {
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() != 5 || columns[0] != "TCP" || columns[4].parse::<u32>().ok() != Some(pid)
        {
            return false;
        }
        let Ok(local) = columns[1].parse::<std::net::SocketAddr>() else {
            return false;
        };
        let Ok(remote) = columns[2].parse::<std::net::SocketAddr>() else {
            return false;
        };
        local.port() == port
            && remote.port() == 0
            && (local.ip().is_unspecified() || local.ip().is_loopback())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_requires_an_open_approved_loopback_port() {
        assert!(approved_server_listener(80).is_err());
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        approved_server_listener(port).unwrap();
        drop(listener);
        assert!(approved_server_listener(port).is_err());
    }
    #[cfg(windows)]
    #[test]
    fn actual_windows_listener_is_bound_to_owned_pid() {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        approved_listener(port, std::process::id()).unwrap();
        assert!(approved_listener(port, std::process::id().wrapping_add(1)).is_err());
    }
    #[test]
    fn listener_requires_exact_pid_and_port_and_no_remote_connection() {
        let text = "TCP 0.0.0.0:54321 0.0.0.0:0 LISTENING 42\nTCP 127.0.0.1:54322 127.0.0.1:54321 ESTABLISHED 42\n";
        assert!(listener_owned(text, 54321, 42));
        assert!(!listener_owned(text, 54321, 43));
        assert!(!listener_owned(text, 54322, 42));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_proc_parser_requires_listen_state_and_local_address() {
        let table = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
             0: 0100007F:D431 00000000:0000 0A 0:0 0:0 0 1000 0 42\n\
             1: 0200007F:D431 00000000:0000 0A 0:0 0:0 0 1000 0 43\n\
             2: 00000000:D431 00000000:0000 01 0:0 0:0 0 1000 0 44\n";
        assert_eq!(
            linux_listening_inodes(table, 0xD431).collect::<Vec<_>>(),
            vec![42]
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn actual_linux_listener_is_bound_to_owned_pid() {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        approved_listener(port, std::process::id()).unwrap();
        assert!(approved_listener(port, std::process::id().wrapping_add(1)).is_err());
        drop(listener);
        assert!(approved_listener(port, std::process::id()).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_child_listener() {
        if std::env::var_os("COBBLE_MESH_PORT_TEST_CHILD").is_none() {
            return;
        }
        use std::io::Read;
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        println!("MESH_PORT_CHILD:{}", listener.local_addr().unwrap().port());
        let _ = std::io::stdin().read_exact(&mut [0u8; 1]);
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_child_listener_requires_exact_game_pid() {
        use std::io::{BufRead, Write};
        use std::process::{Command, Stdio};

        struct OwnedChild(std::process::Child);
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let test_name = format!(
            "{}::linux_child_listener",
            module_path!().split_once("::").unwrap().1
        );
        let mut child = OwnedChild(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &test_name, "--nocapture"])
                .env("COBBLE_MESH_PORT_TEST_CHILD", "1")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id();
        let mut output = std::io::BufReader::new(child.0.stdout.take().unwrap());
        let mut port = None;
        let mut line = String::new();
        while output.read_line(&mut line).unwrap() != 0 {
            if let Some(value) = line.split("MESH_PORT_CHILD:").nth(1) {
                port = value.trim().parse::<u16>().ok();
                break;
            }
            line.clear();
        }
        let port = port.expect("child listener did not report a port");
        approved_listener(port, pid).unwrap();
        owned_listener(port, pid).unwrap();
        assert!(approved_listener(port, std::process::id()).is_err());
        child.0.stdin.take().unwrap().write_all(&[1]).unwrap();
        std::io::copy(&mut output, &mut std::io::sink()).unwrap();
        assert!(child.0.wait().unwrap().success());
        assert!(approved_listener(port, pid).is_err());
    }
}
