use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;

use crate::APP_NAME;

// Macro to generate daemon address constants with validation
#[macro_export]
macro_rules! daemon_addr {
    ($port:expr) => {
        // Compile-time port validation
        const _: () = assert!($port > 1024, "Port must be > 1024 (unprivileged)");
        const _: () = assert!($port <= 65535, "Port must be <= 65535");

        pub const DAEMON_ADDR: &str = concat!("127.0.0.1:", stringify!($port));
    };
}

// Generate the daemon address constants using port 37845
daemon_addr!(37845);

// Command protocol - single byte commands
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Stop = 0x01,
    Show = 0x02,
    Status = 0x03,
}

impl Command {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Command::Stop),
            0x02 => Some(Command::Show),
            0x03 => Some(Command::Status),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

// Response types - identified by first byte
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum ResponseType {
    Ok = 0x80,
    Status = 0x81,
    Error = 0x82,
}

// Response enum
#[derive(Debug)]
pub enum Response {
    Ok(String),
    Error(String),
    Status {
        daemon_running: bool,
        ui_visible: bool,
    },
}

// Standard response messages
pub mod messages {
    pub const DAEMON_STARTED: &str = "Daemon started successfully";
    pub const DAEMON_ALREADY_RUNNING: &str = "Daemon is already running";
    pub const DAEMON_NOT_RUNNING: &str = "Daemon is not running";
    pub const DAEMON_STOPPING: &str = "Daemon stopping";

    pub const UI_LAUNCHED: &str = "UI launched";
    pub const UI_ALREADY_VISIBLE: &str = "UI already visible";

    pub const FAILED_TO_START: &str = "Failed to start daemon";
    pub const FAILED_TO_COMMUNICATE: &str = "Failed to communicate with daemon";
    pub const INVALID_RESPONSE: &str = "Invalid response from daemon";
}

// Send command using byte protocol
pub fn send_command(cmd: Command) -> Result<Response, std::io::Error> {
    let mut stream = TcpStream::connect(DAEMON_ADDR)?;

    // Send single byte command
    stream.write_all(&[cmd.to_byte()])?;
    stream.flush()?;

    // Read response type byte
    let mut resp_type = [0u8; 1];
    stream.read_exact(&mut resp_type)?;

    match resp_type[0] {
        x if x == ResponseType::Ok as u8 => {
            // Ok response
            // Read message length (2 bytes)
            let mut len_buf = [0u8; 2];
            stream.read_exact(&mut len_buf)?;
            let len = u16::from_be_bytes(len_buf) as usize;

            // Read message
            let mut msg_buf = vec![0u8; len];
            stream.read_exact(&mut msg_buf)?;

            Ok(Response::Ok(String::from_utf8_lossy(&msg_buf).into_owned()))
        }
        x if x == ResponseType::Status as u8 => {
            // Status response
            // Read status flags (1 byte)
            let mut flags = [0u8; 1];
            stream.read_exact(&mut flags)?;

            Ok(Response::Status {
                daemon_running: flags[0] & 0x01 != 0,
                ui_visible: flags[0] & 0x02 != 0,
            })
        }
        x if x == ResponseType::Error as u8 => {
            // Error response
            // Read message length (2 bytes)
            let mut len_buf = [0u8; 2];
            stream.read_exact(&mut len_buf)?;
            let len = u16::from_be_bytes(len_buf) as usize;

            // Read error message
            let mut msg_buf = vec![0u8; len];
            stream.read_exact(&mut msg_buf)?;

            Ok(Response::Error(
                String::from_utf8_lossy(&msg_buf).into_owned(),
            ))
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            messages::INVALID_RESPONSE,
        )),
    }
}

// Helper to send a response over the stream
pub fn send_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    match response {
        Response::Ok(msg) => {
            stream.write_all(&[ResponseType::Ok as u8])?;
            let len = (msg.len() as u16).to_be_bytes();
            stream.write_all(&len)?;
            stream.write_all(msg.as_bytes())?;
        }
        Response::Status {
            daemon_running,
            ui_visible,
        } => {
            stream.write_all(&[ResponseType::Status as u8])?;
            let mut flags = 0u8;
            if *daemon_running {
                flags |= 0x01;
            }
            if *ui_visible {
                flags |= 0x02;
            }
            stream.write_all(&[flags])?;
        }
        Response::Error(msg) => {
            stream.write_all(&[ResponseType::Error as u8])?;
            let len = (msg.len() as u16).to_be_bytes();
            stream.write_all(&len)?;
            stream.write_all(msg.as_bytes())?;
        }
    }
    stream.flush()
}

pub fn pid_file_path() -> PathBuf {
    dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(format!("{}.pid", APP_NAME))
}
