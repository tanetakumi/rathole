use anyhow::Result;
use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

/// ポート割り当て管理
pub struct PortAllocator {
    range: Range<u16>,
    allocated: Arc<RwLock<HashSet<u16>>>,
}

impl PortAllocator {
    /// 新しいポートアロケーターを作成
    pub fn new(range: Range<u16>) -> Self {
        Self {
            range,
            allocated: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// 利用可能なポートを割り当て
    pub async fn allocate(&self) -> Result<u16> {
        let mut allocated = self.allocated.write().await;

        // 範囲内で使用されていないポートを順番に探す
        for port in self.range.clone() {
            if !allocated.contains(&port) {
                // 実際にバインド可能か確認
                if self.is_port_available(port).await {
                    allocated.insert(port);
                    return Ok(port);
                }
            }
        }

        anyhow::bail!(
            "No available ports in range {}-{}",
            self.range.start,
            self.range.end
        )
    }

    /// ポートを解放
    pub async fn release(&self, port: u16) {
        self.allocated.write().await.remove(&port);
    }

    /// ポートが実際にバインド可能か確認
    async fn is_port_available(&self, port: u16) -> bool {
        TcpListener::bind(format!("0.0.0.0:{}", port))
            .await
            .is_ok()
    }

    /// 割り当て済みポート数を取得（デバッグ用）
    #[allow(dead_code)]
    pub async fn allocated_count(&self) -> usize {
        self.allocated.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_allocate_and_release() {
        let allocator = PortAllocator::new(35100..35110);

        // ポートを割り当て
        let port1 = allocator.allocate().await.unwrap();
        assert!(port1 >= 35100 && port1 < 35110);

        // 別のポートを割り当て
        let port2 = allocator.allocate().await.unwrap();
        assert!(port2 >= 35100 && port2 < 35110);
        assert_ne!(port1, port2);

        // ポートを解放
        allocator.release(port1).await;

        // 再度割り当て可能
        let port3 = allocator.allocate().await.unwrap();
        assert!(port3 >= 35100 && port3 < 35110);
    }

    #[tokio::test]
    async fn test_port_exhaustion() {
        // 小さな範囲でテスト
        let allocator = PortAllocator::new(35100..35102);

        let port1 = allocator.allocate().await.unwrap();
        let port2 = allocator.allocate().await.unwrap();

        // 3つ目はエラー（範囲は2つのみ）
        let result = allocator.allocate().await;
        assert!(result.is_err());

        // 解放すれば再度割り当て可能
        allocator.release(port1).await;
        let port3 = allocator.allocate().await;
        assert!(port3.is_ok());
    }
}
