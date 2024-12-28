use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const CHUNK_SIZE: usize = 1024;

enum Command {
    List,
    Requests,
    Glide { path: String, to: String },
    Ok(String),
    No(String),
    Help(Option<String>),
    InvalidCommand(String),
}

impl Command {
    fn parse(input: &str) -> Command {
        let glide_re = Regex::new(r"^glide\s+(.+)\s+@(.+)$").unwrap();
        let ok_re = Regex::new(r"^ok\s+@(.+)$").unwrap();
        let no_re = Regex::new(r"^no\s+@(.+)$").unwrap();
        let help_re = Regex::new(r"^help(?:\s+(.+))?$").unwrap();

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
        } else if let Some(caps) = help_re.captures(input) {
            let command = caps.get(1).map(|m| m.as_str().to_string());
            Command::Help(command)
        } else {
            Command::InvalidCommand(input.to_string())
        }
    }

    #[allow(unused)]
    fn get_str(&self) -> Result<String, String> {
        Ok(match self {
            Command::List => "list".to_string(),
            Command::Requests => "reqs".to_string(),
            Command::Glide { path, to } => format!("glide {} @{}", path, to),
            Command::Ok(user) => format!("ok @{}", user),
            Command::No(user) => format!("no @{}", user),
            Command::Help(command) => {
                format!("help {}", command.as_ref().unwrap_or(&String::new()))
                    .trim()
                    .to_string()
            }
            Command::InvalidCommand(s) => return Err(s.to_string()),
        })
    }
}

// Shared state to hold usernames of connected clients
#[derive(Clone, Debug)]
struct Request {
    from_username: String,
    filename: String,
    size: u64,
}

#[derive(Debug)]
struct UserData {
    socket: String,
    incoming_requests: Vec<Request>,
}

type SharedState = Arc<Mutex<HashMap<String, UserData>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server is running on 127.0.0.1:8080");

    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (mut socket, addr) = listener.accept().await?;
        let state = Arc::clone(&state);

        println!("New connection from: {}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut socket, state).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
        });
    }
}

async fn handle_command(
    command: &str,
    username: &str,
    socket: &mut TcpStream,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let command = Command::parse(command);
    match command {
        Command::List => {
            socket.write_all(cmd_list(state).await.as_bytes()).await?;
        }
        Command::Requests => {
            socket
                .write_all(cmd_reqs(state, username).await.as_bytes())
                .await?;
        }
        Command::Glide { path, to } => {
            socket
                .write_all(cmd_glide(state, username, &path, &to).await.as_bytes())
                .await?
        }
        Command::Ok(user) => todo!(),
        Command::No(user) => todo!(),
        Command::Help(_) => {
            let response = "Available commands:\n\
                              list - Show all connected users.\n\
                              help - Show this help message.\n\
                              exit - Disconnect from the server.";
            socket.write_all(response.as_bytes()).await?;
        }
        Command::InvalidCommand(cmd) => {
            let response = format!(
                "Unknown command: {}\nType 'help' for available commands.",
                cmd,
            );
            socket.write_all(response.as_bytes()).await?;
        }
    }

    Ok(())
}

async fn cmd_list(state: &SharedState) -> String {
    let clients = state.lock().await;
    let user_list: Vec<String> = clients
        .keys()
        .cloned()
        .map(|x| format!(" @{}", x))
        .collect();

    if user_list.is_empty() {
        "No users are currently connected.".to_string()
    } else {
        format!("Connected users:\n{}", user_list.join("\n"))
    }
}

async fn cmd_reqs(state: &SharedState, username: &str) -> String {
    let clients = state.lock().await;
    let incoming_user_list: Vec<String> = clients
        .get(username)
        .unwrap()
        .incoming_requests
        .iter()
        .map(|x| {
            format!(
                " @{}, file: {}, size: {} bytes",
                x.from_username, x.filename, x.size,
            )
        })
        .collect();

    if incoming_user_list.is_empty() {
        "No incoming requests".to_string()
    } else {
        format!("Incoming requests:\n{}", incoming_user_list.join("\n"))
    }
}

async fn cmd_glide(state: &SharedState, from: &str, path: &str, to: &str) -> String {
    // Check if file exists
    if !Path::new(path).exists() && fs::metadata(&path).unwrap().is_file() {
        return format!("Path '{}' is invalid. File does not exist", path);
    }

    // Check if user exists
    let mut clients = state.lock().await;
    if !clients.contains_key(to) {
        return format!("User @{} does not exist", to);
    }

    let file_size = fs::metadata(&path).unwrap().size();

    // Add request
    clients
        .get_mut(to)
        .unwrap()
        .incoming_requests
        .push(Request {
            from_username: from.to_string(),
            filename: path.to_string(),
            size: file_size,
        });

    format!("Successfully sent share request to @{} for {}", to, path)
}

async fn handle_client(
    socket: &mut TcpStream,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = vec![0; CHUNK_SIZE];
    let mut username = String::new();

    // Loop until a valid username is provided
    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(()); // Client disconnected
        }

        username.clear();
        username.push_str(
            &String::from_utf8_lossy(&buffer[..bytes_read])
                .trim()
                .to_string(),
        );

        // Check if the username is valid and available
        let response = {
            let clients = state.lock().await;
            if !validate_username(&username) {
                "INVALID_USERNAME"
            } else if clients.contains_key(&username) {
                "USERNAME_TAKEN"
            } else {
                drop(clients);
                add_client(&username, socket, &state).await?;
                "OK"
            }
        };

        // Send the response to the client
        socket.write_all(response.as_bytes()).await?;

        if response == "OK" {
            println!("Client @{} connected", username);
            break;
        }
    }

    // Start command handling loop (e.g., list, help, etc.)
    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            remove_client(&username, &state).await;
            break;
        }

        let command = String::from_utf8_lossy(&buffer[..bytes_read])
            .trim()
            .to_string();

        if let Err(e) = handle_command(&command, &username, socket, &state).await {
            println!("Error handling command for @{}: {}", username, e);
            break;
        }
    }

    Ok(())
}

async fn add_client(
    username: &str,
    socket: &mut TcpStream,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut clients = state.lock().await;
    clients.insert(
        username.to_string(),
        UserData {
            socket: socket.peer_addr()?.to_string(),
            incoming_requests: vec![],
        },
    );
    Ok(())
}

async fn remove_client(username: &str, state: &SharedState) {
    let mut clients = state.lock().await;

    // Remove the client
    clients.remove(username);

    // Collect requests to be removed
    let mut to_remove = Vec::new();
    for (user, client) in clients.iter() {
        for (i, req) in client.incoming_requests.iter().enumerate() {
            if req.from_username == username {
                to_remove.push((user.clone(), i));
            }
        }
    }

    // Remove the collected requests
    for (user, index) in to_remove {
        if let Some(client) = clients.get_mut(&user) {
            client.incoming_requests.remove(index);
        }
    }

    println!("Client @{} disconnected", username);
}

fn validate_username(username: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9\.]{0,8}[a-zA-Z0-9])?$").unwrap();
    re.is_match(username)
}
