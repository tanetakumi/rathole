use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// プロトコルメッセージ（4種類のみ）
#[derive(Serialize, Deserialize, Debug, Clone)]
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
    /// フォーマット: [length: u32][data: bytes]
    pub async fn write_to<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let data = bincode::serialize(self)
            .with_context(|| format!("Failed to serialize message: {:?}", self))?;

        writer
            .write_u32(data.len() as u32)
            .await
            .with_context(|| "Failed to write message length")?;

        writer
            .write_all(&data)
            .await
            .with_context(|| "Failed to write message data")?;

        writer.flush().await?;

        Ok(())
    }

    /// メッセージを受信
    pub async fn read_from<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let len = reader
            .read_u32()
            .await
            .with_context(|| "Failed to read message length")?;

        // メッセージが大きすぎる場合はエラー（DoS対策）
        if len > 1024 * 1024 {
            anyhow::bail!("Message too large: {} bytes", len);
        }

        let mut buf = vec![0u8; len as usize];
        reader
            .read_exact(&mut buf)
            .await
            .with_context(|| "Failed to read message data")?;

        let msg = bincode::deserialize(&buf)
            .with_context(|| "Failed to deserialize message")?;

        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
}
