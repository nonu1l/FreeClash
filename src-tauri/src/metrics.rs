use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use chrono::Utc;
use uuid::Uuid;

use crate::models::{PinConnection, PinStats};

#[derive(Debug)]
struct ConnectionMetric {
    target: String,
    method: String,
    started_at: i64,
    upload: u64,
    download: u64,
    active: bool,
}

#[derive(Debug)]
pub struct PinMetrics {
    node_name: String,
    upload_total: u64,
    download_total: u64,
    last_upload_total: u64,
    last_download_total: u64,
    last_sample: Instant,
    upload_speed: f64,
    download_speed: f64,
    order: VecDeque<String>,
    connections: HashMap<String, ConnectionMetric>,
}

#[derive(Debug, Copy, Clone)]
pub enum TrafficDirection {
    Upload,
    Download,
}

impl PinMetrics {
    pub fn new(node_name: impl Into<String>) -> Self {
        Self {
            node_name: node_name.into(),
            upload_total: 0,
            download_total: 0,
            last_upload_total: 0,
            last_download_total: 0,
            last_sample: Instant::now(),
            upload_speed: 0.0,
            download_speed: 0.0,
            order: VecDeque::new(),
            connections: HashMap::new(),
        }
    }

    pub fn start_connection(&mut self, target: String, method: String) -> String {
        let id = Uuid::new_v4().to_string();
        self.order.push_front(id.clone());
        self.connections.insert(
            id.clone(),
            ConnectionMetric {
                target,
                method,
                started_at: Utc::now().timestamp_millis(),
                upload: 0,
                download: 0,
                active: true,
            },
        );
        self.trim_old_connections();
        id
    }

    pub fn add_bytes(&mut self, connection_id: &str, direction: TrafficDirection, bytes: u64) {
        match direction {
            TrafficDirection::Upload => self.upload_total = self.upload_total.saturating_add(bytes),
            TrafficDirection::Download => {
                self.download_total = self.download_total.saturating_add(bytes)
            }
        }

        if let Some(connection) = self.connections.get_mut(connection_id) {
            match direction {
                TrafficDirection::Upload => {
                    connection.upload = connection.upload.saturating_add(bytes)
                }
                TrafficDirection::Download => {
                    connection.download = connection.download.saturating_add(bytes)
                }
            }
        }
    }

    pub fn finish_connection(&mut self, connection_id: &str) {
        if let Some(connection) = self.connections.get_mut(connection_id) {
            connection.active = false;
        }
        self.trim_old_connections();
    }

    pub fn snapshot(&mut self) -> PinStats {
        let elapsed = self.last_sample.elapsed().as_secs_f64();
        if elapsed >= 0.5 {
            self.upload_speed = (self.upload_total - self.last_upload_total) as f64 / elapsed;
            self.download_speed = (self.download_total - self.last_download_total) as f64 / elapsed;
            self.last_upload_total = self.upload_total;
            self.last_download_total = self.download_total;
            self.last_sample = Instant::now();
        }

        let active_connections = self.connections.values().filter(|conn| conn.active).count();
        let recent_targets = self
            .order
            .iter()
            .filter_map(|id| {
                self.connections.get(id).map(|conn| PinConnection {
                    id: id.clone(),
                    target: conn.target.clone(),
                    method: conn.method.clone(),
                    started_at: conn.started_at,
                    upload: conn.upload,
                    download: conn.download,
                    active: conn.active,
                })
            })
            .take(30)
            .collect();

        PinStats {
            node_name: self.node_name.clone(),
            upload_total: self.upload_total,
            download_total: self.download_total,
            upload_speed: self.upload_speed,
            download_speed: self.download_speed,
            active_connections,
            recent_targets,
        }
    }

    fn trim_old_connections(&mut self) {
        while self.order.len() > 100 {
            let Some(id) = self.order.back().cloned() else {
                break;
            };

            let can_remove = self
                .connections
                .get(&id)
                .map(|connection| !connection.active)
                .unwrap_or(true);

            if can_remove {
                self.order.pop_back();
                self.connections.remove(&id);
            } else {
                break;
            }
        }
    }
}
