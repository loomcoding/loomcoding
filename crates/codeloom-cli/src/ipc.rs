use serde::{de::DeserializeOwned, Serialize};
use std::io;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;

pub trait AsyncStream: AsyncRead + AsyncWrite {}
impl<T: AsyncRead + AsyncWrite> AsyncStream for T {}
pub type Stream = Box<dyn AsyncStream + Unpin + Send>;

pub enum Listener {
    #[cfg(unix)]
    Unix(tokio::net::UnixListener),
    #[cfg(windows)]
    Windows {
        name: String,
        first: Option<tokio::net::windows::named_pipe::NamedPipeServer>,
    },
}

impl Listener {
    pub fn bind() -> io::Result<Self> {
        #[cfg(unix)]
        {
            Self::bind_unix(&runtime_dir())
        }
        #[cfg(windows)]
        {
            Self::bind_named_pipe(pipe_name())
        }
    }

    #[cfg(unix)]
    fn bind_unix(dir: &Path) -> io::Result<Self> {
        {
            use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

            std::fs::create_dir_all(dir)?;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            let path = dir.join("server.sock");
            match tokio::net::UnixListener::bind(&path) {
                Ok(listener) => {
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
                    Ok(Self::Unix(listener))
                }
                Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                    match std::os::unix::net::UnixStream::connect(&path) {
                        Ok(_) => Err(error),
                        Err(connect_error)
                            if matches!(
                                connect_error.kind(),
                                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                            ) =>
                        {
                            let before = std::fs::symlink_metadata(&path)?;
                            if !before.file_type().is_socket() {
                                return Err(error);
                            }
                            let after = std::fs::symlink_metadata(&path)?;
                            if before.dev() != after.dev() || before.ino() != after.ino() {
                                return Err(error);
                            }
                            std::fs::remove_file(&path)?;
                            let listener = tokio::net::UnixListener::bind(&path)?;
                            std::fs::set_permissions(
                                &path,
                                std::fs::Permissions::from_mode(0o600),
                            )?;
                            Ok(Self::Unix(listener))
                        }
                        Err(_) => Err(error),
                    }
                }
                Err(error) => Err(error),
            }
        }
    }

    #[cfg(windows)]
    fn bind_named_pipe(name: String) -> io::Result<Self> {
        let first = create_pipe(&name, true)?;
        Ok(Self::Windows {
            name,
            first: Some(first),
        })
    }

    pub async fn accept(&mut self) -> io::Result<Stream> {
        match self {
            #[cfg(unix)]
            Self::Unix(listener) => listener
                .accept()
                .await
                .map(|(stream, _)| Box::new(stream) as Stream),
            #[cfg(windows)]
            Self::Windows { name, first } => {
                let pipe = match first.take() {
                    Some(pipe) => pipe,
                    None => create_pipe(name, false)?,
                };
                pipe.connect().await?;
                Ok(Box::new(pipe))
            }
        }
    }
}

pub async fn connect() -> io::Result<Stream> {
    #[cfg(unix)]
    {
        tokio::net::UnixStream::connect(runtime_dir().join("server.sock"))
            .await
            .map(|stream| Box::new(stream) as Stream)
    }
    #[cfg(windows)]
    {
        tokio::net::windows::named_pipe::ClientOptions::new()
            .open(pipe_name())
            .map(|stream| Box::new(stream) as Stream)
    }
}

pub fn runtime_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".codeloom")
        .join("runtime")
}

pub fn load_or_create_token() -> io::Result<String> {
    use std::io::Write;

    let dir = runtime_dir();
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let path = dir.join("server.key");
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
    let generated = hex(&random);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            file.write_all(generated.as_bytes())?;
            file.sync_all()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
            Ok(generated)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.file_type().is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "local server key is not a regular file",
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o777 != 0o600 {
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
                }
            }
            let token = std::fs::read_to_string(path)?;
            if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "local server key is malformed",
                ));
            }
            Ok(token)
        }
        Err(error) => Err(error),
    }
}

pub async fn send<W: AsyncWrite + Unpin, T: Serialize>(
    stream: &mut W,
    value: &T,
) -> io::Result<()> {
    let frame = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if frame.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "local IPC frame exceeds limit",
        ));
    }
    stream
        .write_all(&(frame.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&frame).await?;
    stream.flush().await
}

pub async fn receive<T: DeserializeOwned>(stream: &mut (impl AsyncRead + Unpin)) -> io::Result<T> {
    let length = stream.read_u32().await? as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "local IPC frame length is invalid",
        ));
    }
    let mut frame = vec![0; length];
    stream.read_exact(&mut frame).await?;
    serde_json::from_slice(&frame)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(windows)]
fn pipe_name() -> String {
    use std::hash::{Hash, Hasher};
    let profile = dirs::home_dir().unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    profile
        .to_string_lossy()
        .to_ascii_lowercase()
        .hash(&mut hasher);
    format!(r"\\.\pipe\codeloom-{:016x}", hasher.finish())
}

#[cfg(windows)]
fn create_pipe(
    name: &str,
    first: bool,
) -> io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use std::{ffi::c_void, mem::size_of, ptr};
    use tokio::net::windows::named_pipe::ServerOptions;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;

    let sddl: Vec<u16> = "D:P(A;;GA;;;SY)(A;;GA;;;OW)\0".encode_utf16().collect();
    let mut descriptor: *mut c_void = ptr::null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first)
        .max_instances(32)
        .reject_remote_clients(true);
    let result = unsafe {
        options.create_with_security_attributes_raw(
            name,
            (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
        )
    };
    unsafe {
        LocalFree(descriptor);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frames_round_trip_and_reject_oversized_values() {
        let (left, right) = tokio::io::duplex(MAX_FRAME_BYTES * 2);
        let mut writer: Stream = Box::new(left);
        let mut reader: Stream = Box::new(right);
        send(&mut writer, &serde_json::json!({"ok": true}))
            .await
            .unwrap();
        let value: serde_json::Value = receive(&mut reader).await.unwrap();
        assert_eq!(value["ok"], true);

        let too_large = serde_json::json!({"text":"x".repeat(MAX_FRAME_BYTES)});
        assert!(send(&mut writer, &too_large).await.is_err());
    }

    #[test]
    fn tokens_are_64_hex_characters() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0xab;
        let token = hex(&bytes);
        assert_eq!(token, format!("ab{}", "00".repeat(31)));
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_startups_claim_one_socket_and_recover_a_stale_socket() {
        use std::sync::{Arc, Barrier};

        let directory = tempfile::tempdir().unwrap();
        let barrier = Arc::new(Barrier::new(8));
        let attempts = (0..8)
            .map(|_| {
                let directory = directory.path().to_path_buf();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    Listener::bind_unix(&directory)
                })
            })
            .collect::<Vec<_>>();
        let listeners = attempts
            .into_iter()
            .map(|attempt| attempt.join().unwrap())
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        assert_eq!(listeners.len(), 1);
        drop(listeners);
        let restarted = Listener::bind_unix(directory.path()).unwrap();
        drop(restarted);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn named_pipe_singleton_is_claimed_before_the_server_opens_sqlite() {
        let mut random = [0; 8];
        getrandom::fill(&mut random).unwrap();
        let name = format!(r"\\.\pipe\codeloom-test-{}", hex(&random));
        let first = Listener::bind_named_pipe(name.clone()).unwrap();

        assert!(Listener::bind_named_pipe(name.clone()).is_err());

        drop(first);
        assert!(Listener::bind_named_pipe(name).is_ok());
    }
}
