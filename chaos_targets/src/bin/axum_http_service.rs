use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber;

#[derive(Clone)]
struct AppState {
    start_time: Instant,
    orders: Arc<RwLock<Vec<Order>>>,
    order_counter: Arc<RwLock<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    id: u64,
    symbol: String,
    side: OrderSide,
    price: f64,
    quantity: f64,
    status: OrderStatus,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OrderStatus {
    Pending,
    Filled,
    Cancelled,
}

#[derive(Debug, Deserialize)]
struct CreateOrderRequest {
    symbol: String,
    side: OrderSide,
    price: f64,
    quantity: f64,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    uptime_seconds: u64,
    total_orders: usize,
}

#[derive(Debug, Serialize)]
struct MarketData {
    symbol: String,
    price: f64,
    volume: f64,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    let state = AppState {
        start_time: Instant::now(),
        orders: Arc::new(RwLock::new(Vec::new())),
        order_counter: Arc::new(RwLock::new(1)),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/orders", get(list_orders).post(create_order))
        .route("/orders/:id", delete(cancel_order))
        .route("/market-data", get(market_data))
        .with_state(state);

    let addr = "0.0.0.0:8080";
    info!("Starting Axum HTTP service on {}", addr);
    info!("Endpoints:");
    info!("  GET  /health          - Health check");
    info!("  GET  /orders          - List orders");
    info!("  POST /orders          - Create order");
    info!("  DELETE /orders/:id    - Cancel order");
    info!("  GET  /market-data     - Market data");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let orders = state.orders.read().await;
    Json(HealthResponse {
        status: "healthy".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        total_orders: orders.len(),
    })
}

async fn list_orders(State(state): State<AppState>) -> Json<Vec<Order>> {
    let orders = state.orders.read().await;
    Json(orders.clone())
}

async fn create_order(
    State(state): State<AppState>,
    Json(req): Json<CreateOrderRequest>,
) -> (StatusCode, Json<Order>) {
    let mut counter = state.order_counter.write().await;
    let order_id = *counter;
    *counter += 1;

    let order = Order {
        id: order_id,
        symbol: req.symbol,
        side: req.side,
        price: req.price,
        quantity: req.quantity,
        status: OrderStatus::Pending,
        created_at: chrono::Utc::now(),
    };

    let mut orders = state.orders.write().await;
    orders.push(order.clone());

    info!("Created order: {:?}", order);

    (StatusCode::CREATED, Json(order))
}

async fn cancel_order(
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> impl IntoResponse {
    let mut orders = state.orders.write().await;

    if let Some(order) = orders.iter_mut().find(|o| o.id == order_id) {
        order.status = OrderStatus::Cancelled;
        info!("Cancelled order: {}", order_id);
        (StatusCode::OK, Json(order.clone()))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(Order {
                id: 0,
                symbol: String::new(),
                side: OrderSide::Buy,
                price: 0.0,
                quantity: 0.0,
                status: OrderStatus::Cancelled,
                created_at: chrono::Utc::now(),
            }),
        )
    }
}

async fn market_data() -> Json<Vec<MarketData>> {
    // Simulate real-time market data
    let symbols = vec!["BTC/USD", "ETH/USD", "SOL/USD"];
    let data: Vec<MarketData> = symbols
        .iter()
        .map(|symbol| MarketData {
            symbol: symbol.to_string(),
            price: 50000.0 + (rand::random::<f64>() * 1000.0),
            volume: 1000000.0 + (rand::random::<f64>() * 100000.0),
            timestamp: chrono::Utc::now(),
        })
        .collect();

    Json(data)
}
