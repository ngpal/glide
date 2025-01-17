use std::io::{Result, Write};
use std::path::Path;
use tokio::fs::create_dir_all;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::data::CHUNK_SIZE;
use crate::protocol::Transmission;

pub async fn receive_file(stream: &mut TcpStream, save_path: &str) -> Result<()> {
    // Read the first transmission from the stream
    match Transmission::from_stream(stream).await? {
        Transmission::Metadata(filename, file_size) => {
            // Construct the full file path to save the file
            let file_path = format!("{}/{}", save_path, filename);

            // Ensure the parent directories exist
            if let Some(parent_dir) = Path::new(&file_path).parent() {
                create_dir_all(parent_dir).await?;
            }

            // Create the file to save the incoming data
            let mut file = tokio::fs::File::create(file_path).await?;

            let mut total_bytes_received = 0;
            while total_bytes_received < file_size {
                // Read the next chunk of file data from the stream
                match Transmission::from_stream(stream).await? {
                    Transmission::Chunk(chunk_filename, data) if chunk_filename == filename => {
                        // Write the chunk data to the file
                        file.write_all(&data).await?;
                        total_bytes_received += data.len() as u32;

                        // Print progress (optional)
                        print!(
                            "Progress: {}/{} bytes ({:.2}%)\r",
                            total_bytes_received,
                            file_size,
                            total_bytes_received as f64 / file_size as f64 * 100.0
                        );
                        std::io::stdout().flush().unwrap();
                    }
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Unexpected transmission type or mismatched file name",
                        )
                        .into());
                    }
                }
            }

            println!("\nFile transfer completed: {}", filename);
            Ok(())
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Unexpected transmission type, expected Metadata",
        )
        .into()),
    }
}

pub async fn send_file(stream: &mut TcpStream, path: &str) -> Result<()> {
    // Get file metadata
    let metadata = tokio::fs::metadata(path).await?;
    let file_size = metadata.len() as u32;
    let file_name = Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Send metadata as a `Transmission::Metadata` variant
    let metadata_msg = Transmission::Metadata(file_name.clone(), file_size).to_bytes();
    stream.write_all(metadata_msg.as_slice()).await?;

    // Open the file and send its content in chunks
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = vec![0; CHUNK_SIZE]; // Chunk size
    while let Ok(bytes_read) = file.read(&mut buffer).await {
        if bytes_read == 0 {
            break; // End of file
        }

        // Send each chunk as a `Transmission::Chunk` variant
        let chunk_data = buffer[..bytes_read].to_vec();
        let chunk_msg = Transmission::Chunk(file_name.clone(), chunk_data).to_bytes();
        stream.write_all(chunk_msg.as_slice()).await?;
    }

    println!("File sent successfully: {}", file_name);
    Ok(())
}
