# Rathole - シンプル版使い方ガイド

## 概要

このバージョンのratholeは、設定ファイル不要のシンプルなCLIツールとして動作します。

## 主な特徴

- ✅ **設定ファイル不要**: すべてコマンドライン引数で指定
- ✅ **自動ポート割り当て**: サーバー側で35100-35200の範囲から自動割り当て
- ✅ **複数クライアント対応**: 1つのサーバーに複数のクライアントが接続可能
- ✅ **シンプルなAPI**: Rustプログラムから簡単に使用可能
- ✅ **自動再接続**: 接続が切れても自動的に再接続
- ✅ **ハートビート機能**: 接続の健全性を監視

## インストール

```bash
cargo build --release
```

ビルド済みのバイナリは `target/release/rathole` にあります。

## 使い方

### サーバーモード

```bash
# デフォルトアドレス (0.0.0.0:2333) で起動
rathole server

# カスタムアドレスで起動
rathole server 0.0.0.0:8080
```

### クライアントモード

```bash
# ローカルポート8080をリモートサーバーに公開
rathole client myserver.com:2333 8080

# 別の例: SSHポートを公開
rathole client myserver.com:2333 22
```

## 実際の使用例

### 例1: ローカルWebサーバーを公開

```bash
# サーバー側 (例: VPS)
rathole server 0.0.0.0:2333

# クライアント側 (例: 自宅PC)
rathole client vps.example.com:2333 8080

# 出力例:
# Tunnel established! Remote port: 35100
# Press Ctrl+C to stop...

# これで vps.example.com:35100 にアクセスすると
# ローカルの 127.0.0.1:8080 に転送されます
```

### 例2: SSH接続を公開

```bash
# クライアント側
rathole client myserver.com:2333 22

# 別のマシンから接続
ssh user@myserver.com -p 35100
```

### 例3: 複数のサービスを同時に公開

```bash
# クライアント1: Webサーバー
rathole client myserver.com:2333 8080
# → Remote port: 35100

# クライアント2: SSH
rathole client myserver.com:2333 22
# → Remote port: 35101

# クライアント3: データベース
rathole client myserver.com:2333 5432
# → Remote port: 35102
```

## Rustプログラムから使用

ライブラリとして使用することもできます：

```rust
use rathole::start_tunnel;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // トンネルを開始
    let tunnel = start_tunnel("myserver.com:2333", 8080).await?;

    println!("Tunnel established!");
    println!("Remote port: {}", tunnel.remote_port());
    println!("Access via: myserver.com:{}", tunnel.remote_port());

    // プログラムが動作している間トンネルを維持
    tokio::signal::ctrl_c().await?;

    // グレースフルシャットダウン
    tunnel.shutdown().await?;

    Ok(())
}
```

## ポート範囲

サーバーは **35100-35200** の範囲でポートを自動的に割り当てます。

最大100個のクライアントが同時に接続できます。

## ログレベル

環境変数 `RUST_LOG` でログレベルを調整できます：

```bash
# デバッグログを表示
RUST_LOG=debug rathole server

# エラーのみ表示
RUST_LOG=error rathole client myserver.com:2333 8080

# モジュール別に設定
RUST_LOG=rathole=debug,tokio=info rathole server
```

## トラブルシューティング

### 接続できない

1. サーバーのポート2333が開いているか確認
2. ファイアウォールで35100-35200の範囲が開いているか確認
3. `RUST_LOG=debug` でデバッグログを確認

### ポートが枯渇した

サーバー側で100個以上のクライアントを接続しようとした場合、新しいクライアントは接続できません。既存のクライアントを切断してください。

### ローカルサービスに接続できない

クライアント側で指定したローカルポート（例: 8080）にサービスが実際に起動しているか確認してください。

## 従来版との違い

| 機能 | 従来版 | シンプル版 |
|------|--------|------------|
| 設定ファイル | 必須 (TOML) | 不要 |
| 認証 | トークンベース | なし |
| ポート設定 | 手動設定 | 自動割り当て |
| トランスポート | TCP/TLS/Noise/WebSocket | TCPのみ |
| プロトコル | TCP/UDP | TCPのみ |
| コード量 | ~3,147行 | ~830行 |

## セキュリティ上の注意

⚠️ **このバージョンには認証機能がありません**

- 信頼できるネットワーク内でのみ使用してください
- インターネット上で公開する場合は、VPNやSSHトンネルと組み合わせてください
- 本番環境での使用は推奨しません

## ライセンス

Apache License 2.0
