use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    let addr = "0.0.0.0:9000";
    let listener = TcpListener::bind(addr).await?;

    info!("TCP Echo Server listening on {}", addr);
    info!("Telnet to this server and it will echo back everything you send");

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("New connection from {}", addr);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream).await {
                        error!("Error handling connection: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection(mut stream: TcpStream) -> anyhow::Result<()> {
    let peer_addr = stream.peer_addr()?;
    info!("Handling connection from {}", peer_addr);

    // Send welcome message
    stream.write_all(b"Welcome to TCP Echo Server\r\n").await?;

    let mut buffer = [0u8; 4096];
    let mut total_bytes = 0u64;

    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                // Connection closed
                info!(
                    "Connection from {} closed. Total bytes: {}",
                    peer_addr, total_bytes
                );
                break;
            }
            Ok(n) => {
                total_bytes += n as u64;

                // Echo back
                if let Err(e) = stream.write_all(&buffer[..n]).await {
                    error!("Failed to write to socket: {}", e);
                    break;
                }

                // Also log every 1KB
                if total_bytes % 1024 == 0 {
                    info!("{}: {} bytes echoed", peer_addr, total_bytes);
                }
            }
            Err(e) => {
                error!("Error reading from socket: {}", e);
                break;
            }
        }
    }

    Ok(())
}
