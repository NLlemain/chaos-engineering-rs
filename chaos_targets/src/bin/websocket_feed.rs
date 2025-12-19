use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, Level};
use tracing_subscriber;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum MarketMessage {
    OrderBook {
        symbol: String,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    Trade {
        symbol: String,
        price: f64,
        quantity: f64,
        side: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    Ticker {
        symbol: String,
        last_price: f64,
        volume_24h: f64,
        change_24h: f64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceLevel {
    price: f64,
    quantity: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/health", get(health));

    let addr = "0.0.0.0:8081";
    info!("Starting WebSocket Market Data Feed on {}", addr);
    info!("Connect to: ws://localhost:8081/ws");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "WebSocket feed is healthy"
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    info!("New WebSocket connection established");

    let symbols = vec!["BTC/USD", "ETH/USD", "SOL/USD"];
    let mut orderbook_interval = interval(Duration::from_millis(10)); // 100 updates/sec
    let mut trade_interval = interval(Duration::from_millis(20)); // 50 updates/sec
    let mut ticker_interval = interval(Duration::from_millis(100)); // 10 updates/sec

    loop {
        tokio::select! {
            _ = orderbook_interval.tick() => {
                for symbol in &symbols {
                    let msg = generate_orderbook(symbol);
                    if send_message(&mut socket, &msg).await.is_err() {
                        return;
                    }
                }
            }

            _ = trade_interval.tick() => {
                for symbol in &symbols {
                    let msg = generate_trade(symbol);
                    if send_message(&mut socket, &msg).await.is_err() {
                        return;
                    }
                }
            }

            _ = ticker_interval.tick() => {
                for symbol in &symbols {
                    let msg = generate_ticker(symbol);
                    if send_message(&mut socket, &msg).await.is_err() {
                        return;
                    }
                }
            }

            Some(msg) = socket.recv() => {
                match msg {
                    Ok(Message::Close(_)) => {
                        info!("Client disconnected");
                        return;
                    }
                    Ok(Message::Ping(_)) => {
                        if socket.send(Message::Pong(vec![])).await.is_err() {
                            return;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn send_message(socket: &mut WebSocket, msg: &MarketMessage) -> Result<(), ()> {
    let json = serde_json::to_string(msg).map_err(|_| ())?;
    socket.send(Message::Text(json)).await.map_err(|_| ())
}

fn generate_orderbook(symbol: &str) -> MarketMessage {
    let base_price = match symbol {
        "BTC/USD" => 50000.0,
        "ETH/USD" => 3000.0,
        "SOL/USD" => 100.0,
        _ => 100.0,
    };

    let bids: Vec<PriceLevel> = (0..5)
        .map(|i| PriceLevel {
            price: base_price - (i as f64 * base_price * 0.001),
            quantity: 1.0 + (rand::random::<f64>() * 10.0),
        })
        .collect();

    let asks: Vec<PriceLevel> = (0..5)
        .map(|i| PriceLevel {
            price: base_price + (i as f64 * base_price * 0.001),
            quantity: 1.0 + (rand::random::<f64>() * 10.0),
        })
        .collect();

    MarketMessage::OrderBook {
        symbol: symbol.to_string(),
        bids,
        asks,
        timestamp: chrono::Utc::now(),
    }
}

fn generate_trade(symbol: &str) -> MarketMessage {
    let base_price = match symbol {
        "BTC/USD" => 50000.0,
        "ETH/USD" => 3000.0,
        "SOL/USD" => 100.0,
        _ => 100.0,
    };

    MarketMessage::Trade {
        symbol: symbol.to_string(),
        price: base_price * (1.0 + (rand::random::<f64>() - 0.5) * 0.01),
        quantity: rand::random::<f64>() * 10.0,
        side: if rand::random() { "buy" } else { "sell" }.to_string(),
        timestamp: chrono::Utc::now(),
    }
}

fn generate_ticker(symbol: &str) -> MarketMessage {
    let base_price = match symbol {
        "BTC/USD" => 50000.0,
        "ETH/USD" => 3000.0,
        "SOL/USD" => 100.0,
        _ => 100.0,
    };

    MarketMessage::Ticker {
        symbol: symbol.to_string(),
        last_price: base_price * (1.0 + (rand::random::<f64>() - 0.5) * 0.02),
        volume_24h: 1000000.0 + (rand::random::<f64>() * 500000.0),
        change_24h: (rand::random::<f64>() - 0.5) * 10.0,
        timestamp: chrono::Utc::now(),
    }
}
