use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[clap(name = "rathole")]
#[clap(about = "A simple and secure reverse proxy", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// クライアントモード: ローカルポートをリモートサーバーに公開
    Client {
        /// サーバーアドレス (例: myserver.com:2333)
        remote_addr: String,

        /// ローカルポート番号
        local_port: u16,
    },

    /// サーバーモード: クライアント接続を待機
    Server {
        /// バインドアドレス (例: 0.0.0.0:2333)
        #[clap(default_value = "0.0.0.0:2333")]
        bind_addr: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // ロギング設定
    let is_atty = atty::is(atty::Stream::Stdout);
    let level = "info";
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::from(level)),
        )
        .with_ansi(is_atty)
        .init();

    let cli = Cli::parse();

    // Ctrl+Cハンドラー
    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            panic!("Failed to listen for ctrl-c signal: {:?}", e);
        }
        let _ = shutdown_tx.send(());
    });

    match cli.command {
        Commands::Client {
            remote_addr,
            local_port,
        } => {
            let tunnel = rathole::start_tunnel(remote_addr, local_port).await?;
            println!(
                "Tunnel established! Remote port: {}",
                tunnel.remote_port()
            );
            println!("Press Ctrl+C to stop...");

            // シャットダウン待機
            let mut rx = shutdown_rx;
            let _ = rx.recv().await;

            println!("Shutting down...");
            tunnel.shutdown().await?;
        }
        Commands::Server { bind_addr } => {
            rathole::run_server(bind_addr, shutdown_rx).await?;
        }
    }

    Ok(())
}
