use std::io::Write;
use std::{fs, io};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const CHUNK_SIZE: usize = 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Connected to server");

    loop {
        // Get filename
        let mut file_name = String::new();

        print!("Enter the file name to send (or type 'exit' to quit): ");
        io::stdout().flush()?;

        io::stdin().read_line(&mut file_name)?;

        let file_path = file_name.trim();
        if file_path.to_lowercase() == "exit" {
            println!("Exiting...");
            break;
        }

        let file_length;

        // Send file metadata
        match fs::metadata(&file_path) {
            Ok(metadata) => {
                file_length = metadata.len();
                stream
                    .write_all(format!("{}:{}", &file_path, file_length).as_bytes())
                    .await?;
                println!("File metadata sent to the server");
            }
            Err(e) => {
                println!("Error locating the file: {}", e);
                continue;
            }
        }

        // Calculate the number of chunks
        let partial_chunk_size = file_length % CHUNK_SIZE as u64;
        let chunk_count = file_length / CHUNK_SIZE as u64 + (partial_chunk_size > 0) as u64;

        // Read and send chunks
        let mut file = File::open(file_path).await?;
        let mut buffer = vec![0; CHUNK_SIZE];

        for count in 0..chunk_count {
            let bytes_read = file.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }

            stream.write_all(&buffer[..bytes_read]).await?;
            println!(
                "Sent chunk {}/{} ({}%)",
                count + 1,
                chunk_count,
                ((count + 1) as f64 / chunk_count as f64 * 100.0) as u8
            );
        }

        println!("File transfer completed successfully!");
    }

    Ok(())
}
