use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// プロトコルメッセージ（4種類のみ）
/// JSON形式でシリアライズされ、言語非依存
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Message {
    /// クライアント → サーバー: トンネル作成リクエスト
    TunnelRequest { local_port: u16 },

    /// サーバー → クライアント: 割り当てたポート番号
    TunnelResponse { assigned_port: u16 },

    /// サーバー → クライアント: データチャネルを作成して
    CreateDataChannel,

    /// 双方向: ハートビート
    Heartbeat,
}

impl Message {
    /// メッセージを送信
    /// フォーマット: [length: u32 little-endian][json_data: UTF-8 bytes]
    pub async fn write_to<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        // JSONに変換
        let json = serde_json::to_string(self)
            .with_context(|| format!("Failed to serialize message to JSON: {:?}", self))?;
        let data = json.as_bytes();

        // length (u32, little-endian)
        writer
            .write_u32_le(data.len() as u32)
            .await
            .with_context(|| "Failed to write message length")?;

        // JSON data
        writer
            .write_all(data)
            .await
            .with_context(|| "Failed to write message data")?;

        writer.flush().await?;

        Ok(())
    }

    /// メッセージを受信
    pub async fn read_from<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        // length (u32, little-endian)
        let len = reader
            .read_u32_le()
            .await
            .with_context(|| "Failed to read message length")?;

        // メッセージが大きすぎる場合はエラー（DoS対策）
        if len > 1024 * 1024 {
            anyhow::bail!("Message too large: {} bytes", len);
        }

        // JSON data
        let mut buf = vec![0u8; len as usize];
        reader
            .read_exact(&mut buf)
            .await
            .with_context(|| "Failed to read message data")?;

        // JSONからデシリアライズ
        let json = String::from_utf8(buf)
            .with_context(|| "Failed to convert message data to UTF-8")?;

        let msg = serde_json::from_str(&json)
            .with_context(|| format!("Failed to deserialize JSON: {}", json))?;

        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_roundtrip() {
        let messages = vec![
            Message::TunnelRequest { local_port: 8080 },
            Message::TunnelResponse { assigned_port: 35100 },
            Message::CreateDataChannel,
            Message::Heartbeat,
        ];

        for msg in messages {
            let mut buf = Vec::new();
            msg.write_to(&mut buf).await.unwrap();

            let mut cursor = std::io::Cursor::new(buf);
            let decoded = Message::read_from(&mut cursor).await.unwrap();

            // メッセージが正しくエンコード/デコードされることを確認
            match (msg, decoded) {
                (Message::TunnelRequest { local_port: p1 }, Message::TunnelRequest { local_port: p2 }) => {
                    assert_eq!(p1, p2);
                }
                (Message::TunnelResponse { assigned_port: p1 }, Message::TunnelResponse { assigned_port: p2 }) => {
                    assert_eq!(p1, p2);
                }
                (Message::CreateDataChannel, Message::CreateDataChannel) => {}
                (Message::Heartbeat, Message::Heartbeat) => {}
                _ => panic!("Message mismatch"),
            }
        }
    }

    #[tokio::test]
    async fn test_json_format() {
        // JSONフォーマットが正しいか確認
        let msg = Message::TunnelRequest { local_port: 8080 };
        let mut buf = Vec::new();
        msg.write_to(&mut buf).await.unwrap();

        // lengthの4バイトをスキップしてJSON部分を取得
        let json_data = &buf[4..];
        let json_str = String::from_utf8(json_data.to_vec()).unwrap();

        // JSONとしてパース可能か確認
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "TunnelRequest");
        assert_eq!(parsed["local_port"], 8080);
    }
}
