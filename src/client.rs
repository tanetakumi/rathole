use anyhow::{Context, Result};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::protocol::Message;

const RETRY_INTERVAL: Duration = Duration::from_secs(3);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);

/// サーバーに接続してポート番号を取得
pub async fn connect_and_get_port(
    remote_addr: String,
    local_port: u16,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<u16> {
    loop {
        tokio::select! {
            result = try_connect(&remote_addr, local_port) => {
                match result {
                    Ok(port) => return Ok(port),
                    Err(e) => {
                        warn!("Connection failed: {}, retrying in {:?}...", e, RETRY_INTERVAL);
                        tokio::time::sleep(RETRY_INTERVAL).await;
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                return Err(anyhow::anyhow!("Shutdown requested"));
            }
        }
    }
}

/// クライアントを実行（メインループ）
pub async fn run_client(
    remote_addr: String,
    local_port: u16,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<()> {
    loop {
        tokio::select! {
            result = try_run_client(&remote_addr, local_port) => {
                match result {
                    Ok(_) => {
                        info!("Client disconnected normally");
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Client error: {}, retrying in {:?}...", e, RETRY_INTERVAL);
                        tokio::time::sleep(RETRY_INTERVAL).await;
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Client shutdown requested");
                return Ok(());
            }
        }
    }
}

/// サーバーに接続を試行
async fn try_connect(remote_addr: &str, local_port: u16) -> Result<u16> {
    debug!("Connecting to server: {}", remote_addr);

    let mut stream = TcpStream::connect(remote_addr)
        .await
        .with_context(|| format!("Failed to connect to {}", remote_addr))?;

    // トンネル作成をリクエスト
    Message::TunnelRequest { local_port }
        .write_to(&mut stream)
        .await
        .context("Failed to send TunnelRequest")?;

    // 割り当てられたポートを受信
    let assigned_port = timeout(Duration::from_secs(10), Message::read_from(&mut stream))
        .await
        .context("Timeout waiting for TunnelResponse")??;

    match assigned_port {
        Message::TunnelResponse { assigned_port } => {
            info!("Tunnel established! Remote port: {}", assigned_port);
            Ok(assigned_port)
        }
        _ => Err(anyhow::anyhow!("Unexpected response from server")),
    }
}

/// クライアント実行を試行
async fn try_run_client(remote_addr: &str, local_port: u16) -> Result<()> {
    debug!("Starting client for {}:{}", remote_addr, local_port);

    let mut stream = TcpStream::connect(remote_addr)
        .await
        .with_context(|| format!("Failed to connect to {}", remote_addr))?;

    // トンネル作成をリクエスト
    Message::TunnelRequest { local_port }
        .write_to(&mut stream)
        .await
        .context("Failed to send TunnelRequest")?;

    // 割り当てられたポートを受信
    let response = timeout(Duration::from_secs(10), Message::read_from(&mut stream))
        .await
        .context("Timeout waiting for TunnelResponse")??;

    let assigned_port = match response {
        Message::TunnelResponse { assigned_port } => assigned_port,
        _ => return Err(anyhow::anyhow!("Unexpected response from server")),
    };

    info!("Connected! Remote port: {}", assigned_port);

    // コントロールチャネルループ
    control_channel_loop(stream, remote_addr.to_string(), local_port).await
}

/// コントロールチャネルのメインループ
async fn control_channel_loop(
    mut stream: TcpStream,
    remote_addr: String,
    local_port: u16,
) -> Result<()> {
    let mut heartbeat_interval = tokio::time::interval(HEARTBEAT_INTERVAL);

    loop {
        tokio::select! {
            // サーバーからのメッセージを受信
            msg_result = timeout(HEARTBEAT_TIMEOUT, Message::read_from(&mut stream)) => {
                match msg_result {
                    Ok(Ok(msg)) => {
                        match msg {
                            Message::CreateDataChannel => {
                                debug!("Received CreateDataChannel request");
                                // データチャネルを非同期で作成
                                let remote_addr_clone = remote_addr.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = create_data_channel(remote_addr_clone, local_port).await {
                                        error!("Data channel error: {}", e);
                                    }
                                });
                            }
                            Message::Heartbeat => {
                                debug!("Received heartbeat");
                                // ハートビートを返す
                                Message::Heartbeat.write_to(&mut stream).await?;
                            }
                            _ => {
                                warn!("Unexpected message: {:?}", msg);
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        return Err(e).context("Failed to read message");
                    }
                    Err(_) => {
                        return Err(anyhow::anyhow!("Heartbeat timeout"));
                    }
                }
            }

            // 定期的にハートビートを送信
            _ = heartbeat_interval.tick() => {
                debug!("Sending heartbeat");
                if let Err(e) = Message::Heartbeat.write_to(&mut stream).await {
                    return Err(e).context("Failed to send heartbeat");
                }
            }
        }
    }
}

/// データチャネルを作成
async fn create_data_channel(remote_addr: String, local_port: u16) -> Result<()> {
    debug!("Creating data channel to {}", remote_addr);

    // サーバーに接続
    let server_stream = TcpStream::connect(&remote_addr)
        .await
        .with_context(|| format!("Failed to connect to server at {}", remote_addr))?;

    // ローカルサービスに接続
    let local_stream = TcpStream::connect(format!("127.0.0.1:{}", local_port))
        .await
        .with_context(|| format!("Failed to connect to local service at port {}", local_port))?;

    debug!("Data channel established, starting bidirectional copy");

    // 双方向コピー
    let (mut server_read, mut server_write) = tokio::io::split(server_stream);
    let (mut local_read, mut local_write) = tokio::io::split(local_stream);

    let client_to_server = tokio::io::copy(&mut local_read, &mut server_write);
    let server_to_client = tokio::io::copy(&mut server_read, &mut local_write);

    tokio::select! {
        result = client_to_server => {
            match result {
                Ok(bytes) => debug!("Client -> Server: {} bytes", bytes),
                Err(e) => debug!("Client -> Server error: {}", e),
            }
        }
        result = server_to_client => {
            match result {
                Ok(bytes) => debug!("Server -> Client: {} bytes", bytes),
                Err(e) => debug!("Server -> Client error: {}", e),
            }
        }
    }

    debug!("Data channel closed");
    Ok(())
}
