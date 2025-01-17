use tokio::{
    io::{AsyncReadExt, Result},
    net::TcpStream,
};

use crate::{commands::Command, data::Request};

#[derive(Debug)]
pub enum Transmission {
    Username(String),
    UsernameOk,
    UsernameTaken,
    UsernameInvalid,
    Command(Command),
    GlideRequestSent,
    Metadata(String, u32),
    Chunk(String, Vec<u8>),
    ConnectedUsers(Vec<String>),
    IncomingRequests(Vec<Request>),
    OkSuccess,
    OkFailed,
    NoSuccess,
    ClientDisconnected,
}

impl Transmission {
    pub fn to_bytes(&self) -> Vec<u8> {
        match *self {
            Self::Username(ref user) => Vec::from(format!("\u{1}{}\0", user)),
            Self::UsernameOk => vec![2],
            Self::UsernameTaken => vec![3],
            Self::UsernameInvalid => vec![4],
            Self::Metadata(ref filename, size) => {
                let mut ret = Vec::from(format!("\u{5}{}\0", filename));
                size.to_be_bytes().iter().for_each(|&b| ret.push(b));

                ret
            }
            Self::Chunk(ref filename, ref data) => {
                let chunk_size = data.len() as u16;
                let chunk_size_bytes = chunk_size.to_be_bytes();
                let mut ret = Vec::from(format!("\u{6}{}\0", filename,));

                chunk_size_bytes.iter().for_each(|&b| ret.push(b));
                ret.extend(data);

                ret
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
                .into()
            }
            Self::IncomingRequests(ref requests) => {
                let num_requests = requests.len() as u16;
                let num_requests_bytes = num_requests.to_be_bytes();
                let requests_str: String = requests
                    .iter()
                    .map(|req| format!("{}\0{}", req.sender, req.filename))
                    .collect::<Vec<_>>()
                    .join("\0");
                format!(
                    "\u{8}{}{}\0",
                    String::from_utf8_lossy(&num_requests_bytes),
                    requests_str
                )
                .into()
            }
            Self::Command(ref cmd) => match cmd {
                Command::List => vec![9, 1],
                Command::Requests => vec![9, 2],
                Command::Glide {
                    path,
                    to: ref username,
                } => format!("\u{9}\u{3}{}\0{}\0", path, username).into(),
                Command::Ok(ref username) => format!("\u{9}\u{4}{}\0", username).into(),
                Command::No(ref username) => format!("\u{9}\u{4}{}\0", username).into(),
            },
            Self::OkFailed => vec![10],
            Self::NoSuccess => vec![11],
            Self::ClientDisconnected => vec![12],
            Self::GlideRequestSent => vec![13],
            Self::OkSuccess => vec![14],
        }
    }

    pub async fn from_stream(stream: &mut TcpStream) -> Result<Transmission> {
        loop {
            let first_byte = stream.read_u8().await?; // get the first byte (control byte)

            return match first_byte {
                0x0 => continue,
                0x1 => {
                    // username
                    let mut username = String::new();
                    loop {
                        let ch = stream.read_u8().await? as char;
                        if ch == '\0' {
                            break;
                        }
                        username.push(ch);
                    }
                    Ok(Self::Username(username))
                }
                0x2 => Ok(Self::UsernameOk),
                0x3 => Ok(Self::UsernameTaken),
                0x4 => Ok(Self::UsernameInvalid),
                0x5 => {
                    // metadata
                    let mut filename = String::new();
                    loop {
                        let ch = stream.read_u8().await? as char;
                        if ch == '\0' {
                            break;
                        }
                        filename.push(ch);
                    }
                    let mut size_bytes = [0u8; 4];
                    stream.read_exact(&mut size_bytes).await?;
                    let size = u32::from_be_bytes(size_bytes);

                    Ok(Self::Metadata(filename, size))
                }
                0x6 => {
                    // chunk
                    let mut filename = String::new();
                    loop {
                        let ch = stream.read_u8().await? as char;
                        if ch == '\0' {
                            break;
                        }
                        filename.push(ch);
                    }
                    let mut chunk_size_bytes = [0u8; 2];
                    stream.read_exact(&mut chunk_size_bytes).await?;
                    let chunk_size = u16::from_be_bytes(chunk_size_bytes);

                    let mut data = vec![0u8; chunk_size as usize];
                    stream.read_exact(&mut data).await?;

                    Ok(Self::Chunk(filename, data))
                }
                0x7 => {
                    // connected users
                    let mut num_users_bytes = [0u8; 2];
                    stream.read_exact(&mut num_users_bytes).await?;
                    let num_users = u16::from_be_bytes(num_users_bytes);

                    let mut users = Vec::new();
                    for _ in 0..num_users {
                        let mut user = String::new();
                        loop {
                            let ch = stream.read_u8().await? as char;
                            if ch == '\0' {
                                break;
                            }
                            user.push(ch);
                        }
                        users.push(user);
                    }

                    Ok(Self::ConnectedUsers(users))
                }
                0x8 => {
                    // incoming requests
                    let mut num_requests_bytes = [0u8; 2];
                    stream.read_exact(&mut num_requests_bytes).await?;
                    let num_requests = u16::from_be_bytes(num_requests_bytes);

                    let mut requests = Vec::new();
                    for _ in 0..num_requests {
                        let mut sender = String::new();
                        loop {
                            let ch = stream.read_u8().await? as char;
                            if ch == '\0' {
                                break;
                            }
                            sender.push(ch);
                        }

                        let mut filename = String::new();
                        loop {
                            let ch = stream.read_u8().await? as char;
                            if ch == '\0' {
                                break;
                            }
                            filename.push(ch);
                        }

                        requests.push(Request { sender, filename });
                    }

                    Ok(Self::IncomingRequests(requests))
                }
                0x9 => {
                    // command
                    let command_type = stream.read_u8().await?;
                    match command_type {
                        1 => Ok(Self::Command(Command::List)),
                        2 => Ok(Self::Command(Command::Requests)),
                        3 => {
                            let mut path = String::new();
                            loop {
                                let ch = stream.read_u8().await? as char;
                                if ch == '\0' {
                                    break;
                                }
                                path.push(ch);
                            }
                            let mut username = String::new();
                            loop {
                                let ch = stream.read_u8().await? as char;
                                if ch == '\0' {
                                    break;
                                }
                                username.push(ch);
                            }
                            Ok(Self::Command(Command::Glide { path, to: username }))
                        }
                        4 => {
                            let mut username = String::new();
                            loop {
                                let ch = stream.read_u8().await? as char;
                                if ch == '\0' {
                                    break;
                                }
                                username.push(ch);
                            }
                            Ok(Self::Command(Command::Ok(username)))
                        }
                        5 => {
                            let mut username = String::new();
                            loop {
                                let ch = stream.read_u8().await? as char;
                                if ch == '\0' {
                                    break;
                                }
                                username.push(ch);
                            }
                            Ok(Self::Command(Command::No(username)))
                        }
                        something => panic!("what is this command {}", something),
                    }
                }
                0xa => Ok(Self::OkFailed),
                0xb => Ok(Self::NoSuccess),
                0xc => Ok(Self::ClientDisconnected),
                0xd => Ok(Self::GlideRequestSent),
                0xe => Ok(Self::OkSuccess),
                something => {
                    let mut wrong = [0u8; 1024];
                    wrong[0] = something;

                    stream.read(&mut wrong[1..]).await?;
                    panic!("somethings really wrong :( {:#?}", wrong);
                }
            };
        }
    }
}
