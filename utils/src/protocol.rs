use crate::commands::Command;

pub enum Transmission {
    Username(String),
    UsernameOk,
    UsernameTaken,
    UsernameInvalid,
    Command(Command),
    Metadata(String, u32),
    Chunk(String, String),
    ConnectedUsers(Vec<String>),
    IncomingRequests(Vec<(String, String)>),
    OkFailed,
    NoSuccess,
    ClientDisconnected,
}

impl Transmission {
    pub fn to_string(&self) -> String {
        match *self {
            Self::Username(ref user) => format!("\u{1}{}\0", user),
            Self::UsernameOk => String::from("\u{2}"),
            Self::UsernameTaken => String::from("\u{3}"),
            Self::UsernameInvalid => String::from("\u{4}"),
            Self::Metadata(ref filename, size) => {
                let size_bytes = size.to_be_bytes();
                format!(
                    "\u{5}{}\0{}",
                    filename,
                    String::from_utf8_lossy(&size_bytes)
                )
            }
            Self::Chunk(ref filename, ref data) => {
                let chunk_size = data.len() as u16;
                let chunk_size_bytes = chunk_size.to_be_bytes();
                format!(
                    "\u{6}{}\0{}{}",
                    filename,
                    String::from_utf8_lossy(&chunk_size_bytes),
                    data
                )
            }
            Self::ConnectedUsers(ref users) => {
                let num_users = users.len() as u16;
                let num_users_bytes = num_users.to_be_bytes();
                let users_str = users.join("\0");
                format!(
                    "\u{7}{}{}\0{}",
                    String::from_utf8_lossy(&num_users_bytes),
                    users_str,
                    "\0"
                )
            }
            Self::IncomingRequests(ref requests) => {
                let num_requests = requests.len() as u16;
                let num_requests_bytes = num_requests.to_be_bytes();
                let requests_str: String = requests
                    .iter()
                    .map(|(from, filename)| format!("{}\0{}", from, filename))
                    .collect::<Vec<_>>()
                    .join("\0");
                format!(
                    "\u{8}{}{}\0",
                    String::from_utf8_lossy(&num_requests_bytes),
                    requests_str
                )
            }
            Self::Command(ref cmd) => match cmd {
                Command::List => format!("\u{9}\u{1}"),
                Command::Requests => format!("\u{9}\u{2}"),
                Command::Glide {
                    path,
                    to: ref username,
                } => {
                    format!("\u{9}\u{3}{}\0{}\0", path, username)
                }
                Command::Ok(ref username) => format!("\u{9}\u{4}{}\0", username),
                Command::No(ref username) => format!("\u{9}\u{4}{}\0", username),
                _ => unreachable!(),
            },
            Self::OkFailed => String::from("\u{10}"),
            Self::NoSuccess => String::from("\u{11}"),
            Self::ClientDisconnected => String::from("\u{12}"),
        }
    }

    pub fn from_string(input: &str) -> Option<Self> {
        let first_byte = input.chars().next().unwrap_or('\0') as u8; // Get the first byte (control byte)

        match first_byte {
            1 => {
                // Username
                let parts: Vec<&str> = input[1..].split('\0').collect();
                if parts.len() == 1 {
                    Some(Self::Username(parts[0].to_string()))
                } else {
                    None
                }
            }
            2 => Some(Self::UsernameOk),
            3 => Some(Self::UsernameTaken),
            4 => Some(Self::UsernameInvalid),
            5 => {
                // Metadata
                let parts: Vec<&str> = input[1..].split('\0').collect();
                if parts.len() == 2 {
                    let filename = parts[0].to_string();
                    let size_bytes = parts[1].as_bytes();
                    if size_bytes.len() == 4 {
                        let size = u32::from_be_bytes([
                            size_bytes[0],
                            size_bytes[1],
                            size_bytes[2],
                            size_bytes[3],
                        ]);
                        Some(Self::Metadata(filename, size))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            6 => {
                // Chunk
                let parts: Vec<&str> = input[1..].split('\0').collect();
                if parts.len() == 2 {
                    let filename = parts[0].to_string();
                    let chunk_size_bytes = parts[1].as_bytes();
                    if chunk_size_bytes.len() == 2 {
                        let data = &input[1 + filename.len() + 2..]; // Start after the filename and 2-byte chunk size
                        Some(Self::Chunk(filename, data.to_string()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            7 => {
                // ConnectedUsers
                let parts: Vec<&str> = input[1..].split('\0').collect();
                if parts.len() > 1 {
                    let num_users =
                        u16::from_be_bytes([parts[0].as_bytes()[0], parts[0].as_bytes()[1]]);
                    if num_users == (parts.len() - 1) as u16 {
                        let users = parts[1..].iter().map(|s| s.to_string()).collect();
                        Some(Self::ConnectedUsers(users))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            8 => {
                // IncomingRequests
                let parts: Vec<&str> = input[1..].split('\0').collect();
                if parts.len() > 1 {
                    let num_requests =
                        u16::from_be_bytes([parts[0].as_bytes()[0], parts[0].as_bytes()[1]]);
                    if num_requests == (parts.len() - 1) as u16 / 2 {
                        let requests = parts[1..]
                            .chunks(2)
                            .map(|chunk| (chunk[0].to_string(), chunk[1].to_string()))
                            .collect();
                        Some(Self::IncomingRequests(requests))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            9 => {
                // Command
                let command_code = input[1..2].chars().next().unwrap_or('\0') as u8;
                let username_end = input[2..].find('\0').unwrap_or(input.len());
                let username = &input[2..2 + username_end];
                match command_code {
                    1 => Some(Self::Command(Command::List)),
                    2 => Some(Self::Command(Command::Requests)),
                    3 => {
                        // Glide command
                        let parts: Vec<&str> = input[2..].split('\0').collect();
                        if parts.len() == 2 {
                            Some(Self::Command(Command::Glide {
                                path: parts[0].to_string(),
                                to: parts[1].to_string(),
                            }))
                        } else {
                            None
                        }
                    }
                    4 => Some(Self::Command(Command::Ok(username.to_string()))),
                    _ => None,
                }
            }
            10 => Some(Self::OkFailed),
            11 => Some(Self::NoSuccess),
            12 => Some(Self::ClientDisconnected),
            _ => None,
        }
    }
}
