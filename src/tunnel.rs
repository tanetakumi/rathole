use anyhow::Result;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::client;

/// 確立されたトンネル
pub struct Tunnel {
    remote_addr: String,
    local_port: u16,
    assigned_port: u16,
    shutdown_tx: broadcast::Sender<()>,
    handle: JoinHandle<Result<()>>,
}

impl Tunnel {
    /// 割り当てられたリモートポートを取得
    pub fn remote_port(&self) -> u16 {
        self.assigned_port
    }

    /// リモートアドレスを取得
    pub fn remote_addr(&self) -> &str {
        &self.remote_addr
    }

    /// ローカルポートを取得
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    /// トンネルをシャットダウン
    pub async fn shutdown(self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        self.handle.await??;
        Ok(())
    }
}

/// トンネルを開始（メインAPI）
///
/// # 引数
/// * `remote_addr` - サーバーアドレス (例: "myserver.com:2333")
/// * `local_port` - ローカルポート番号
///
/// # 戻り値
/// 確立されたトンネル。サーバーから割り当てられたポート番号を含む。
///
/// # 例
/// ```no_run
/// use rathole::start_tunnel;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let tunnel = start_tunnel("myserver.com:2333", 8080).await?;
///     println!("Remote port: {}", tunnel.remote_port());
///
///     // プログラム実行中...
///     tokio::signal::ctrl_c().await?;
///
///     tunnel.shutdown().await?;
///     Ok(())
/// }
/// ```
pub async fn start_tunnel(
    remote_addr: impl Into<String>,
    local_port: u16,
) -> Result<Tunnel> {
    let remote_addr = remote_addr.into();
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    // クライアントを起動してポート番号を取得
    let assigned_port = client::connect_and_get_port(
        remote_addr.clone(),
        local_port,
        shutdown_rx.resubscribe(),
    )
    .await?;

    // バックグラウンドでクライアントを実行
    let remote_addr_clone = remote_addr.clone();
    let shutdown_rx_clone = shutdown_rx.resubscribe();
    let handle = tokio::spawn(async move {
        client::run_client(remote_addr_clone, local_port, shutdown_rx_clone).await
    });

    Ok(Tunnel {
        remote_addr,
        local_port,
        assigned_port,
        shutdown_tx,
        handle,
    })
}
