use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server is running on 127.0.0.1:8080");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("New connection from: {}", addr);

        tokio::spawn(async move {
            let mut buffer = [0; 1024];

            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) => {
                        println!("Client has closed the connection.");
                        break;
                    }
                    Ok(bytes_read) => println!(
                        "Recieved: {}",
                        String::from_utf8_lossy(&buffer[..bytes_read])
                    ),
                    Err(e) => {
                        eprintln!("Failed to read from socket: {}", e);
                        break;
                    }
                }
            }
        });
    }
}
