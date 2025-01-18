use crate::{
    data::{Request, UserData},
    protocol::Transmission,
    transfers,
};
use regex::Regex;
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::Mutex};

type SharedState = Arc<Mutex<HashMap<String, UserData>>>;

#[derive(Clone, Debug)]
pub enum Command {
    List,
    Requests,
    Glide { path: String, to: String },
    Ok(String),
    No(String),
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
            unreachable!("oh no")
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Command::List => "list".to_string(),
            Command::Requests => "reqs".to_string(),
            Command::Glide { path, to } => format!("glide {} @{}", path, to),
            Command::Ok(user) => format!("ok @{}", user),
            Command::No(user) => format!("no @{}", user),
        }
    }

    pub async fn execute(&self, state: &SharedState, username: &str) -> Transmission {
        match self {
            Command::List => self.cmd_list(state, username).await,
            Command::Requests => self.cmd_reqs(state, username).await,
            Command::Glide { path: _, to: _ } => self.cmd_glide(state, username).await,
            Command::Ok(_) => self.cmd_ok(state, username).await,
            Command::No(_) => self.cmd_no(state, username).await,
        }
    }

    // Executes and prints the output of a command to a user
    pub async fn handle(
        command: Command,
        username: &str,
        stream: &mut TcpStream,
        state: &SharedState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let response = command.execute(state, username).await;
        stream.write_all(response.to_bytes().as_slice()).await?;

        // If the reponse was GlideRequestSent, receive file
        if matches!(response, Transmission::GlideRequestSent) {
            // Create a directory to save the incoming data
            let Command::Glide { to, .. } = command else {
                unreachable!("the command should always be glide")
            };
            let file_path = format!("{}/{}", username, to);

            // Ensure the parent directories exist
            if let Some(parent_dir) = std::path::Path::new(&file_path).parent() {
                tokio::fs::create_dir_all(parent_dir).await?;
            }

            transfers::receive_file(stream, &file_path).await?;
        } else if matches!(response, Transmission::OkSuccess) {
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
                            sender: from_username,
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

            let path = format!("clients/{}/{}/{}", from, username, filename);

            transfers::send_file(stream, &path).await?;

            // Remove the file after sending
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }

    // -- Command implementations --

    async fn cmd_list(&self, state: &SharedState, username: &str) -> Transmission {
        let clients = state.lock().await;
        let user_list: Vec<String> = clients.keys().cloned().filter(|x| x != username).collect();

        Transmission::ConnectedUsers(user_list)
    }

    async fn cmd_reqs(&self, state: &SharedState, username: &str) -> Transmission {
        let clients = state.lock().await;
        let incoming_user_list: Vec<Request> =
            clients.get(username).unwrap().incoming_requests.clone();

        Transmission::IncomingRequests(incoming_user_list)
    }

    async fn cmd_glide(&self, state: &SharedState, username: &str) -> Transmission {
        let Command::Glide { path, to } = self else {
            unreachable!()
        };

        // Check if user exists
        let mut clients = state.lock().await;
        if !clients.contains_key(to) || username == to {
            return Transmission::UsernameInvalid;
        }

        // Add request
        clients
            .get_mut(to)
            .unwrap()
            .incoming_requests
            .push(Request {
                sender: username.to_string(),
                filename: Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            });

        Transmission::GlideRequestSent
    }

    async fn cmd_ok(&self, state: &SharedState, username: &str) -> Transmission {
        let Command::Ok(from) = self else {
            unreachable!()
        };

        let clients = state.lock().await;

        if let Some(client) = clients.get(username) {
            let valid_request = client
                .incoming_requests
                .iter()
                .any(|req| &req.sender == from);

            if valid_request {
                return Transmission::OkSuccess;
            }
        }

        Transmission::OkFailed
    }

    async fn cmd_no(&self, state: &SharedState, username: &str) -> Transmission {
        let Command::No(from) = self else {
            unreachable!()
        };

        let mut clients = state.lock().await;

        if let Some(client) = clients.get_mut(username) {
            if let Some(pos) = client
                .incoming_requests
                .iter()
                .position(|req| &req.sender == from)
            {
                let request = client.incoming_requests.remove(pos);
                let file_path = format!("clients/{}/{}/{}", from, username, request.filename);
                let _ = tokio::fs::remove_file(file_path).await; // ignore errors
            }
        }

        Transmission::NoSuccess
    }
}
