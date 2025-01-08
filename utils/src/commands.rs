use regex::Regex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
};

use crate::data::{Request, ServerResponse, UserData, CHUNK_SIZE};
use std::{collections::HashMap, sync::Arc};

type SharedState = Arc<Mutex<HashMap<String, UserData>>>;

pub enum Command {
    List,
    Requests,
    Glide { path: String, to: String },
    Ok(String),
    No(String),
    InvalidCommand(String),
}

impl Command {
    pub fn parse(input: &str) -> Command {
        let glide_re = Regex::new(r"^glide\s+(.+)\s+@(.+)$").unwrap();
        let ok_re = Regex::new(r"^ok\s+@(.+)$").unwrap();
        let no_re = Regex::new(r"^no\s+@(.+)$").unwrap();

        if input == "list" {
            Command::List
        } else if input == "reqs" {
            Command::Requests
        } else if let Some(caps) = glide_re.captures(input) {
            let path = caps[1].to_string();
            let to = caps[2].to_string();
            Command::Glide { path, to }
        } else if let Some(caps) = ok_re.captures(input) {
            let username = caps[1].to_string();
            Command::Ok(username)
        } else if let Some(caps) = no_re.captures(input) {
            let username = caps[1].to_string();
            Command::No(username)
        } else {
            Command::InvalidCommand(input.to_string())
        }
    }

    pub fn get_str(&self) -> Result<String, String> {
        Ok(match self {
            Command::List => "list".to_string(),
            Command::Requests => "reqs".to_string(),
            Command::Glide { path, to } => format!("glide {} @{}", path, to),
            Command::Ok(user) => format!("ok @{}", user),
            Command::No(user) => format!("no @{}", user),
            Command::InvalidCommand(s) => return Err(s.to_string()),
        })
    }

    pub async fn execute(&self, state: &SharedState, username: &str) -> ServerResponse {
        match self {
            Command::List => self.cmd_list(state, username).await,
            Command::Requests => self.cmd_reqs(state, username).await,
            Command::Glide { path: _, to: _ } => self.cmd_glide(state, username).await,
            Command::Ok(_) => self.cmd_ok(state, username).await,
            Command::No(_) => self.cmd_no(state, username).await,
            Command::InvalidCommand(_) => ServerResponse::UnknownCommand,
        }
    }

    // Executes and prints the output of a command to a user
    pub async fn handle(
        command: &str,
        username: &str,
        socket: &mut TcpStream,
        state: &SharedState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let command = Command::parse(command);
        let response = command.execute(state, username).await;
        socket.write_all(response.to_string().as_bytes()).await?;

        // If the reponse was GlideRequestSent, receive file
        if matches!(response, ServerResponse::GlideRequestSent) {
            let mut buffer = vec![0; CHUNK_SIZE];

            // Read metadata (file name and size)
            let bytes_read = socket.read(&mut buffer).await?;
            if bytes_read == 0 {
                return Ok(()); // Client disconnected
            }

            // Extract metadata
            let (file_name, file_size) = {
                let metadata = String::from_utf8_lossy(&buffer[..bytes_read]);
                let parts: Vec<&str> = metadata.split(':').collect();
                if parts.len() != 2 {
                    return Err("Invalid metadata format".into());
                }
                let file_name = parts[0].trim().to_string();
                let file_size: u64 = parts[1].trim().parse()?;
                (file_name, file_size)
            };
            println!("Receiving file: {} ({} bytes)", file_name, file_size);

            // Get to username
            let Command::Glide { to, .. } = command else {
                unreachable!("the command should always be glide")
            };

            // Create a file to save the incoming data
            let mut file =
                tokio::fs::File::create(format!("{}/{}/{}", username, to, &file_name)).await?;

            // Receive chunks and write to file
            let mut total_bytes_received = 0;
            while total_bytes_received < file_size {
                let bytes_read = socket.read(&mut buffer).await?;
                if bytes_read == 0 {
                    println!("Client disconnected unexpectedly");
                    break;
                }

                file.write_all(&buffer[..bytes_read]).await?;
                total_bytes_received += bytes_read as u64;
                println!(
                    "Progress: {}/{} bytes ({:.2}%)",
                    total_bytes_received,
                    file_size,
                    total_bytes_received as f64 / file_size as f64 * 100.0
                );
            }
            println!("File transfer completed: {}", file_name);
        }

        Ok(())
    }

    // -- Command implementations --

    async fn cmd_list(&self, state: &SharedState, username: &str) -> ServerResponse {
        let clients = state.lock().await;
        let user_list: Vec<String> = clients.keys().cloned().filter(|x| x != username).collect();

        ServerResponse::ConnectedUsers(user_list)
    }

    async fn cmd_reqs(&self, state: &SharedState, username: &str) -> ServerResponse {
        let clients = state.lock().await;
        let incoming_user_list: Vec<Request> =
            clients.get(username).unwrap().incoming_requests.clone();

        ServerResponse::IncomingRequests(incoming_user_list)
    }

    async fn cmd_glide(&self, state: &SharedState, username: &str) -> ServerResponse {
        let Command::Glide { path, to } = self else {
            unreachable!()
        };

        // Check if user exists
        let mut clients = state.lock().await;
        if !clients.contains_key(to) || username == to {
            return ServerResponse::UnknownCommand;
        }

        // Add request
        clients
            .get_mut(to)
            .unwrap()
            .incoming_requests
            .push(Request {
                from_username: username.to_string(),
                filename: path.to_string(),
            });

        ServerResponse::GlideRequestSent
    }

    async fn cmd_ok(&self, state: &SharedState, username: &str) -> ServerResponse {
        // When the Ok command is sent, we check if the Ok is valid, and let the handler
        // do the rest

        todo!()
    }

    async fn cmd_no(&self, state: &SharedState, username: &str) -> ServerResponse {
        todo!()
    }
}
