use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::metrics::{RuleMetrics, TrafficDirection};

const HEADER_LIMIT: usize = 64 * 1024;

pub async fn start_meter_proxy(
    rule_name: String,
    listen_port: u16,
    upstream_port: u16,
    metrics: Arc<Mutex<RuleMetrics>>,
) -> Result<JoinHandle<()>> {
    let listener = TcpListener::bind(("127.0.0.1", listen_port))
        .await
        .with_context(|| format!("无法监听计量端口 {listen_port}"))?;

    Ok(tokio::spawn(async move {
        if let Err(err) = accept_loop(listener, rule_name.clone(), upstream_port, metrics).await {
            eprintln!("计量代理 {rule_name} 已停止：{err:#}");
        }
    }))
}

async fn accept_loop(
    listener: TcpListener,
    rule_name: String,
    upstream_port: u16,
    metrics: Arc<Mutex<RuleMetrics>>,
) -> Result<()> {
    loop {
        let (client, _) = listener.accept().await?;
        let metrics = metrics.clone();
        let rule_name = rule_name.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_client(client, upstream_port, metrics).await {
                eprintln!("meter proxy for {rule_name} failed: {err:#}");
            }
        });
    }
}

async fn handle_client(
    mut client: TcpStream,
    upstream_port: u16,
    metrics: Arc<Mutex<RuleMetrics>>,
) -> Result<()> {
    let mut first_packet = Vec::new();
    let request = read_initial_request(&mut client, &mut first_packet).await?;
    let mut upstream = TcpStream::connect(("127.0.0.1", upstream_port))
        .await
        .with_context(|| format!("无法连接 mihomo 上游端口 {upstream_port}"))?;

    upstream.write_all(&first_packet).await?;

    let connection_id = {
        let mut guard = metrics.lock().map_err(|_| anyhow!("计量状态锁已损坏"))?;
        let id = guard.start_connection(request.target, request.method);
        guard.add_bytes(&id, TrafficDirection::Upload, first_packet.len() as u64);
        id
    };

    let (mut client_reader, mut client_writer) = client.into_split();
    let (mut upstream_reader, mut upstream_writer) = upstream.into_split();

    let upload_metrics = metrics.clone();
    let upload_id = connection_id.clone();
    let upload = tokio::spawn(async move {
        copy_metered(
            &mut client_reader,
            &mut upstream_writer,
            upload_metrics,
            upload_id,
            TrafficDirection::Upload,
        )
        .await
    });

    let download_metrics = metrics.clone();
    let download_id = connection_id.clone();
    let download = tokio::spawn(async move {
        copy_metered(
            &mut upstream_reader,
            &mut client_writer,
            download_metrics,
            download_id,
            TrafficDirection::Download,
        )
        .await
    });

    let _ = tokio::join!(upload, download);
    if let Ok(mut guard) = metrics.lock() {
        guard.finish_connection(&connection_id);
    }
    Ok(())
}

async fn copy_metered<R, W>(
    reader: &mut R,
    writer: &mut W,
    metrics: Arc<Mutex<RuleMetrics>>,
    connection_id: String,
    direction: TrafficDirection,
) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = vec![0_u8; 16 * 1024];
    loop {
        let bytes = reader.read(&mut buffer).await?;
        if bytes == 0 {
            let _ = writer.shutdown().await;
            return Ok(());
        }
        writer.write_all(&buffer[..bytes]).await?;
        if let Ok(mut guard) = metrics.lock() {
            guard.add_bytes(&connection_id, direction, bytes as u64);
        }
    }
}

#[derive(Debug)]
struct InitialRequest {
    method: String,
    target: String,
}

async fn read_initial_request(client: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<InitialRequest> {
    let mut chunk = [0_u8; 4096];
    loop {
        let bytes = client.read(&mut chunk).await?;
        if bytes == 0 {
            return Err(anyhow!("客户端在发送代理请求前已断开"));
        }
        buffer.extend_from_slice(&chunk[..bytes]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            return parse_initial_request(buffer);
        }
        if buffer.len() > HEADER_LIMIT {
            return Err(anyhow!("HTTP 代理请求头超过 64KiB"));
        }
    }
}

fn parse_initial_request(buffer: &[u8]) -> Result<InitialRequest> {
    let header_end = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| anyhow!("未找到完整 HTTP 请求头"))?;
    let header = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header.lines();
    let request_line = lines.next().ok_or_else(|| anyhow!("缺少 HTTP 请求行"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("UNKNOWN").to_string();
    let uri = parts.next().unwrap_or("").to_string();

    let host = lines
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("host") {
                Some(value.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let target = if method.eq_ignore_ascii_case("CONNECT") {
        if uri.is_empty() {
            host
        } else {
            uri
        }
    } else if uri.starts_with("http://") || uri.starts_with("https://") {
        uri
    } else if host.is_empty() {
        uri
    } else {
        format!("{host}{uri}")
    };

    Ok(InitialRequest { method, target })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connect_target() {
        let parsed = parse_initial_request(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n").unwrap();
        assert_eq!(parsed.method, "CONNECT");
        assert_eq!(parsed.target, "example.com:443");
    }

    #[test]
    fn parses_http_url() {
        let parsed = parse_initial_request(b"GET http://example.com/path HTTP/1.1\r\nHost: example.com\r\n\r\n").unwrap();
        assert_eq!(parsed.method, "GET");
        assert_eq!(parsed.target, "http://example.com/path");
    }
}
