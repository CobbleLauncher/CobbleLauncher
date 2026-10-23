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
    #[cfg(not(windows))]
    {
        let _ = (pid, probe);
        Err("此平台尚不支持验证游戏端口所属进程".into())
    }
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
}
