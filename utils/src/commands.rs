use regex::Regex;
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::Mutex};

use crate::data::{Request, ServerResponse, UserData};
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

    pub async fn execute(&self, state: &SharedState, username: &str) -> String {
        match self {
            Command::List => self.cmd_list(state, username).await,
            Command::Requests => self.cmd_reqs(state, username).await,
            Command::Glide { path: _, to: _ } => self.cmd_glide(state, username).await,
            Command::Ok(_) => self.cmd_ok(state, username).await,
            Command::No(_) => self.cmd_no(state, username).await,
            Command::InvalidCommand(cmd) => {
                return format!(
                    "Unknown command: {}\nType 'help' for available commands.",
                    cmd,
                )
            }
        }
        .to_string()
    }

    // Executes and prints the output of a command to a user
    pub async fn handle(
        command: &str,
        username: &str,
        socket: &mut TcpStream,
        state: &SharedState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let response = Command::parse(command).execute(state, username).await;
        socket.write_all(response.as_bytes()).await?;

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
        let (path, to) = match self {
            Command::Glide { path, to } => (path, to),
            _ => unreachable!(),
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
        todo!()
    }

    async fn cmd_no(&self, state: &SharedState, username: &str) -> ServerResponse {
        todo!()
    }
}
