use regex::Regex;
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
};

use crate::data::{Request, ServerResponse, UserData, CHUNK_SIZE};
use std::{collections::HashMap, path::Path, sync::Arc};

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
        stream: &mut TcpStream,
        state: &SharedState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let command = Command::parse(command);
        let response = command.execute(state, username).await;
        stream.write_all(response.to_string().as_bytes()).await?;

        // If the reponse was GlideRequestSent, receive file
        if matches!(response, ServerResponse::GlideRequestSent) {
            let mut buffer = vec![0; CHUNK_SIZE];

            // Read metadata (file name and size)
            let bytes_read = stream.read(&mut buffer).await?;
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

            let file_path = format!("{}/{}/{}", username, to, file_name);

            // Ensure the parent directories exist
            if let Some(parent_dir) = std::path::Path::new(&file_path).parent() {
                tokio::fs::create_dir_all(parent_dir).await?;
            }

            // Now create the file
            let mut file = tokio::fs::File::create(file_path).await?;

            // Receive chunks and write to file
            let mut total_bytes_received = 0;
            while total_bytes_received < file_size {
                let bytes_read = stream.read(&mut buffer).await?;
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
        } else if matches!(response, ServerResponse::OkSuccess) {
            dbg!("Sending file...");

            // Get the request
            let Command::Ok(from) = command else {
                unreachable!();
            };

            let filename = {
                let clients = state.lock().await;

                if let Some(requests) = clients.get(username).map(|c| &c.incoming_requests) {
                    // use a labeled loop for breaking with a value
                    'outer: loop {
                        for Request {
                            from_username,
                            filename,
                        } in requests.iter()
                        {
                            if from_username == &from {
                                // break with the value
                                break 'outer filename.clone();
                            }
                        }

                        unreachable!()
                    }
                } else {
                    unreachable!()
                }
            };

            let path = &format!("{}/{}/{}", from, username, filename);

            // Send file over to the user
            let metadata = fs::metadata(&path).await?;
            let file_length = metadata.len();

            // Send metadata
            stream
                .write_all(
                    format!(
                        "{}:{}",
                        Path::new(&path).file_name().unwrap().to_string_lossy(),
                        file_length
                    )
                    .as_bytes(),
                )
                .await?;
            println!("Metadata sent!");

            // Calculate the number of chunks
            let partial_chunk_size = file_length % CHUNK_SIZE as u64;
            let chunk_count = file_length / CHUNK_SIZE as u64 + (partial_chunk_size > 0) as u64;

            // Read and send chunks
            let mut file = File::open(&path).await?;
            let mut buffer = vec![0; CHUNK_SIZE];
            for _ in 0..chunk_count {
                let bytes_read = file.read(&mut buffer).await?;
                if bytes_read == 0 {
                    break;
                }
                stream.write_all(&buffer[..bytes_read]).await?;
            }

            println!("\nFile sent successfully!");

            // Remove the file
            tokio::fs::remove_file(format!("{}/{}/{}", from, username, filename)).await?;
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
                filename: Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            });

        ServerResponse::GlideRequestSent
    }

    async fn cmd_ok(&self, state: &SharedState, username: &str) -> ServerResponse {
        let Command::Ok(from) = self else {
            unreachable!()
        };

        let clients = state.lock().await;

        if let Some(client) = clients.get(username) {
            let valid_request = client
                .incoming_requests
                .iter()
                .any(|req| &req.from_username == from);

            if valid_request {
                return ServerResponse::OkSuccess;
            }
        }

        ServerResponse::OkFailed
    }

    async fn cmd_no(&self, state: &SharedState, username: &str) -> ServerResponse {
        let Command::No(from) = self else {
            unreachable!()
        };

        let mut clients = state.lock().await;

        if let Some(client) = clients.get_mut(username) {
            if let Some(pos) = client
                .incoming_requests
                .iter()
                .position(|req| &req.from_username == from)
            {
                let request = client.incoming_requests.remove(pos);
                let file_path = format!("{}/{}/{}", from, username, request.filename);
                let _ = tokio::fs::remove_file(file_path).await; // ignore errors
            }
        }

        ServerResponse::NoSuccess
    }
}
