use anyhow::{Context, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::port_allocator::PortAllocator;
use crate::protocol::Message;

const PORT_RANGE_START: u16 = 35100;
const PORT_RANGE_END: u16 = 35200;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);

/// クライアント情報
struct ClientInfo {
    assigned_port: u16,
    data_channel_tx: mpsc::Sender<TcpStream>,
    control_channel_tx: mpsc::Sender<Message>,
}

/// サーバーを実行
pub async fn run_server(bind_addr: String, mut shutdown_rx: broadcast::Receiver<()>) -> Result<()> {
    let listener = TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("Failed to bind to {}", bind_addr))?;

    info!("Server listening on {}", bind_addr);
    info!("Port range: {}-{}", PORT_RANGE_START, PORT_RANGE_END);

    let port_allocator = Arc::new(PortAllocator::new(PORT_RANGE_START..PORT_RANGE_END));
    let clients: Arc<RwLock<HashMap<SocketAddr, ClientInfo>>> = Arc::new(RwLock::new(HashMap::new()));

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        debug!("New connection from {}", addr);
                        let allocator = port_allocator.clone();
                        let clients = clients.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, addr, allocator, clients).await {
                                error!("Connection error from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Server shutdown requested");
                return Ok(());
            }
        }
    }
}

/// 接続を処理
async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    allocator: Arc<PortAllocator>,
    clients: Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
) -> Result<()> {
    // 最初のメッセージを受信
    let msg = timeout(Duration::from_secs(10), Message::read_from(&mut stream))
        .await
        .context("Timeout waiting for initial message")??;

    match msg {
        Message::TunnelRequest { local_port } => {
            // 新しいコントロールチャネル
            handle_control_channel(stream, addr, local_port, allocator, clients).await
        }
        _ => {
            // データチャネルとして処理
            handle_data_channel(stream, addr, clients).await
        }
    }
}

/// コントロールチャネルを処理
async fn handle_control_channel(
    mut stream: TcpStream,
    addr: SocketAddr,
    local_port: u16,
    allocator: Arc<PortAllocator>,
    clients: Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
) -> Result<()> {
    info!("Control channel from {} (local port: {})", addr, local_port);

    // ポートを割り当て
    let assigned_port = allocator
        .allocate()
        .await
        .context("Failed to allocate port")?;

    info!("Assigned port {} to {}", assigned_port, addr);

    // ポートでリスナー起動
    let listener = TcpListener::bind(format!("0.0.0.0:{}", assigned_port))
        .await
        .with_context(|| format!("Failed to bind to port {}", assigned_port))?;

    // レスポンス送信
    Message::TunnelResponse { assigned_port }
        .write_to(&mut stream)
        .await
        .context("Failed to send TunnelResponse")?;

    info!("Tunnel established for {} on port {}", addr, assigned_port);

    // データチャネルキュー
    let (data_tx, mut data_rx) = mpsc::channel::<TcpStream>(32);

    // コントロールメッセージチャネル
    let (control_tx, mut control_rx) = mpsc::channel::<Message>(32);

    // クライアント情報を保存
    {
        let mut clients = clients.write().await;
        clients.insert(
            addr,
            ClientInfo {
                assigned_port,
                data_channel_tx: data_tx,
                control_channel_tx: control_tx,
            },
        );
    }

    // 訪問者接続を待機するタスク
    let data_tx_clone = {
        let clients = clients.read().await;
        clients.get(&addr).map(|info| info.control_channel_tx.clone())
    };

    if let Some(control_tx) = data_tx_clone {
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((visitor_stream, visitor_addr)) => {
                        info!("Visitor connected to port {} from {}", assigned_port, visitor_addr);

                        // クライアントにデータチャネル作成を要求
                        if let Err(e) = control_tx.send(Message::CreateDataChannel).await {
                            error!("Failed to request data channel: {}", e);
                            break;
                        }

                        // データチャネルが来るまで待機
                        match tokio::time::timeout(Duration::from_secs(10), data_rx.recv()).await {
                            Ok(Some(data_stream)) => {
                                // 訪問者とデータチャネルを接続
                                tokio::spawn(async move {
                                    if let Err(e) = forward_traffic(visitor_stream, data_stream).await {
                                        debug!("Traffic forwarding error: {}", e);
                                    }
                                });
                            }
                            Ok(None) => {
                                error!("Data channel closed");
                                break;
                            }
                            Err(_) => {
                                warn!("Timeout waiting for data channel");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to accept visitor: {}", e);
                        break;
                    }
                }
            }
            info!("Listener for port {} stopped", assigned_port);
        });
    }

    // ハートビートループ
    let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);

    loop {
        tokio::select! {
            // クライアントからのメッセージを受信
            msg_result = Message::read_from(&mut stream) => {
                match msg_result {
                    Ok(Message::Heartbeat) => {
                        debug!("Received heartbeat from {}", addr);
                        Message::Heartbeat.write_to(&mut stream).await?;
                    }
                    Ok(msg) => {
                        warn!("Unexpected message from {}: {:?}", addr, msg);
                    }
                    Err(e) => {
                        info!("Control channel closed for {}: {}", addr, e);
                        break;
                    }
                }
            }

            // 内部からのコントロールメッセージを送信
            Some(msg) = control_rx.recv() => {
                if let Err(e) = msg.write_to(&mut stream).await {
                    warn!("Failed to send message to {}: {}", addr, e);
                    break;
                }
            }

            // 定期的にハートビートを送信
            _ = heartbeat_interval.tick() => {
                debug!("Sending heartbeat to {}", addr);
                if let Err(e) = Message::Heartbeat.write_to(&mut stream).await {
                    warn!("Failed to send heartbeat to {}: {}", addr, e);
                    break;
                }
            }
        }
    }

    // クリーンアップ
    info!("Cleaning up client {}", addr);
    {
        let mut clients = clients.write().await;
        if let Some(client_info) = clients.remove(&addr) {
            allocator.release(client_info.assigned_port).await;
            info!("Released port {}", client_info.assigned_port);
        }
    }

    Ok(())
}

/// データチャネルを処理
async fn handle_data_channel(
    stream: TcpStream,
    addr: SocketAddr,
    clients: Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
) -> Result<()> {
    debug!("Data channel from {}", addr);

    // クライアント情報を取得
    let data_tx = {
        let clients = clients.read().await;
        clients
            .get(&addr)
            .map(|info| info.data_channel_tx.clone())
    };

    if let Some(data_tx) = data_tx {
        // データチャネルをキューに追加
        data_tx
            .send(stream)
            .await
            .context("Failed to send data channel")?;
        debug!("Data channel queued for {}", addr);
    } else {
        warn!("No control channel found for data channel from {}", addr);
    }

    Ok(())
}

/// トラフィックを転送
async fn forward_traffic(visitor: TcpStream, data: TcpStream) -> Result<()> {
    let (mut visitor_read, mut visitor_write) = tokio::io::split(visitor);
    let (mut data_read, mut data_write) = tokio::io::split(data);

    let visitor_to_data = tokio::io::copy(&mut visitor_read, &mut data_write);
    let data_to_visitor = tokio::io::copy(&mut data_read, &mut visitor_write);

    tokio::select! {
        result = visitor_to_data => {
            match result {
                Ok(bytes) => debug!("Visitor -> Data: {} bytes", bytes),
                Err(e) => debug!("Visitor -> Data error: {}", e),
            }
        }
        result = data_to_visitor => {
            match result {
                Ok(bytes) => debug!("Data -> Visitor: {} bytes", bytes),
                Err(e) => debug!("Data -> Visitor error: {}", e),
            }
        }
    }

    Ok(())
}
