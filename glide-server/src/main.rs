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

    // Shared state for tracking connected clients
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (mut socket, addr) = listener.accept().await?;
        let state = Arc::clone(&state); // Clone the state for each connection

        println!("New connection from: {}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut socket, state).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(
    socket: &mut TcpStream,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = vec![0; CHUNK_SIZE];

    // Step 1: Register the client with a username
    let bytes_read = socket.read(&mut buffer).await?;
    if bytes_read == 0 {
        return Ok(()); // Client disconnected
    }

    // Extract the username
    let username = String::from_utf8_lossy(&buffer[..bytes_read]).trim().to_string();
    {
        let mut clients = state.lock().await;
        if clients.contains_key(&username) {
            socket.write_all(b"Username already taken").await?;
            return Err("Username conflict".into());
        }
        clients.insert(username.clone(), socket.peer_addr()?.to_string());
    }

    println!("Client '{}' connected", username);
    socket.write_all(b"Welcome to the server!\n").await?;

    // Step 2: Handle file transfer
    let bytes_read = socket.read(&mut buffer).await?;
    if bytes_read == 0 {
        remove_client(&username, &state).await;
        return Ok(()); // Client disconnected
    }

    // Extract metadata
    let (file_name, file_size) = {
        let metadata = String::from_utf8_lossy(&buffer[..bytes_read]);
        let parts: Vec<&str> = metadata.split(':').collect();
        if parts.len() != 2 {
            remove_client(&username, &state).await;
            return Err("Invalid metadata format".into());
        }
        let file_name = parts[0].trim().to_string();
        let file_size: u64 = parts[1].trim().parse()?;
        (file_name, file_size)
    };

    println!("Receiving file '{}' from '{}' ({} bytes)", file_name, username, file_size);

    let mut file = tokio::fs::File::create("new_".to_owned() + &file_name).await?;
    let mut total_bytes_received = 0;

    while total_bytes_received < file_size {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            println!("Client '{}' disconnected unexpectedly", username);
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
    Ok(())
}

async fn remove_client(username: &str, state: &SharedState) {
    let mut clients = state.lock().await;
    clients.remove(username);
    println!("Client '{}' disconnected", username);
}
