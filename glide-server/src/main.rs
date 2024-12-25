use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const CHUNK_SIZE: usize = 1024;

// Shared state to hold usernames of connected clients
type SharedState = Arc<Mutex<HashMap<String, String>>>;

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
    _username: &str,
    socket: &mut TcpStream,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    match command.trim() {
        "list" => {
            // Fetch the list of all connected usernames
            let clients = state.lock().await;
            let user_list: Vec<String> = clients
                .keys()
                .cloned()
                .map(|x| format!(" @{}", x))
                .collect();

            let response = if user_list.is_empty() {
                "No users are currently connected.".to_string()
            } else {
                format!("Connected users:\n{}", user_list.join("\n"))
            };

            socket.write_all(response.as_bytes()).await?;
        }
        "help" => {
            let response = "Available commands:\n\
                              list - Show all connected users.\n\
                              help - Show this help message.\n\
                              exit - Disconnect from the server.";
            socket.write_all(response.as_bytes()).await?;
        }
        _ => {
            let response = format!(
                "Unknown command: {}\nType 'help' for available commands.",
                command
            );
            socket.write_all(response.as_bytes()).await?;
        }
    }

    Ok(())
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
            let mut clients = state.lock().await;
            if !validate_username(&username) {
                "INVALID_USERNAME"
            } else if clients.contains_key(&username) {
                "USERNAME_TAKEN"
            } else {
                clients.insert(username.clone(), socket.peer_addr()?.to_string());
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

async fn remove_client(username: &str, state: &SharedState) {
    let mut clients = state.lock().await;
    clients.remove(username);
    println!("Client @{} disconnected", username);
}

fn validate_username(username: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9](?:[a-zA-Z0-9\.]{0,8}[a-zA-Z0-9])?$").unwrap();
    re.is_match(username)
}
