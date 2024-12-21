use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const CHUNK_SIZE: usize = 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server is running on 127.0.0.1:8080");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("New connection from: {}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_client(&mut socket).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(socket: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
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

    // Create a file to save the incoming data
    let mut file = tokio::fs::File::create("new_".to_owned() + &file_name).await?;

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
    Ok(())
}
