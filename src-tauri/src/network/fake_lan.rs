use crate::error::{AppError, AppResult};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::Notify;

pub const MC_LAN_MULTICAST_ADDR: &str = "224.0.2.60:4445";

pub struct FakeLanBroadcaster {
    stop_notify: Arc<Notify>,
    pub local_port: u16,
    pub motd: String,
}

impl FakeLanBroadcaster {
    /// Start broadcasting `[MOTD]§a[P2P] Friend's Server[/MOTD][AD]<dynamic_port>[/AD]`
    /// to UDP multicast `224.0.2.60:4445` every 1.5 seconds.
    pub async fn start(local_port: u16, custom_motd: Option<String>) -> AppResult<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
            .await
            .map_err(|e| AppError::P2p(format!("Failed to bind Fake LAN UDP socket: {}", e)))?;

        let _ = socket.set_multicast_loop_v4(true);
        let _ = socket.set_multicast_ttl_v4(4);
        let _ = socket.set_broadcast(true);

        let motd = custom_motd.unwrap_or_else(|| "§a[P2P] Friend's Server".to_string());
        let payload = format!("[MOTD]{}[/MOTD][AD]{}[/AD]", motd, local_port);
        let stop_notify = Arc::new(Notify::new());
        let stop_clone = stop_notify.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1500));
            loop {
                tokio::select! {
                    _ = stop_clone.notified() => {
                        break;
                    }
                    _ = interval.tick() => {
                        let _ = socket.send_to(payload.as_bytes(), MC_LAN_MULTICAST_ADDR).await;
                    }
                }
            }
        });

        Ok(Self {
            stop_notify,
            local_port,
            motd,
        })
    }

    pub fn stop(&self) {
        self.stop_notify.notify_waiters();
    }
}

impl Drop for FakeLanBroadcaster {
    fn drop(&mut self) {
        self.stop_notify.notify_waiters();
    }
}
