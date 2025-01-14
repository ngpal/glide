use std::io::Result;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::data::CHUNK_SIZE;

pub async fn receive_file(stream: &mut TcpStream, save_path: &str) -> Result<()> {
    let mut buffer = vec![0; CHUNK_SIZE]; // Use an appropriate chunk size
                                          // Read the file name (null-terminated)
    let mut filename_buffer = Vec::new();
    loop {
        let mut byte = [0; 1];
        stream.read_exact(&mut byte).await?;
        if byte[0] == b'\0' {
            break; // Null-terminated file name
        }
        filename_buffer.push(byte[0]);
    }
    let file_name = String::from_utf8(filename_buffer).expect("File name is messed up :/");

    // Read the 4-byte file size
    let mut size_buffer = [0; 4];
    stream.read_exact(&mut size_buffer).await?;
    let file_size = u32::from_be_bytes(size_buffer) as u64;

    // Construct the full file path to save the file
    let file_path = format!("{}/{}", save_path, file_name);

    // Ensure the parent directories exist
    if let Some(parent_dir) = Path::new(&file_path).parent() {
        tokio::fs::create_dir_all(parent_dir).await?;
    }

    // Create the file to save the incoming data
    let mut file = tokio::fs::File::create(file_path).await?;

    // Receive the file content in chunks
    let mut total_bytes_received = 0;
    while total_bytes_received < file_size {
        let bytes_to_read =
            std::cmp::min(buffer.len() as u64, file_size - total_bytes_received) as usize;
        let bytes_read = stream.read(&mut buffer[..bytes_to_read]).await?;
        if bytes_read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Client disconnected unexpectedly",
            )
            .into());
        }

        file.write_all(&buffer[..bytes_read]).await?;
        total_bytes_received += bytes_read as u64;

        // Print progress (optional)
        println!(
            "Progress: {}/{} bytes ({:.2}%)\r",
            total_bytes_received,
            file_size,
            total_bytes_received as f64 / file_size as f64 * 100.0
        );
    }

    println!("\nFile transfer completed: {}", file_name);
    Ok(())
}

pub async fn send_file(stream: &mut tokio::net::TcpStream, path: &str) -> Result<()> {
    // Get file metadata
    let metadata = tokio::fs::metadata(path).await?;
    let file_size = metadata.len();
    let file_name = Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Send the file name (null-terminated)
    stream.write_all(file_name.as_bytes()).await?;
    stream.write_all(&[0]).await?; // Null terminator

    // Send the 4-byte file size in binary
    let file_size_bytes = (file_size as u32).to_be_bytes();
    stream.write_all(&file_size_bytes).await?;

    // Send file content in chunks
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = vec![0; CHUNK_SIZE];
    let mut total_bytes_sent = 0;

    while total_bytes_sent < file_size {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        stream.write_all(&buffer[..bytes_read]).await?;
        total_bytes_sent += bytes_read as u64;
    }

    println!("File sent successfully: {}", file_name);
    Ok(())
}
