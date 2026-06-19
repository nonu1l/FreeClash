use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::metrics::{PinMetrics, TrafficDirection};

const HEADER_LIMIT: usize = 64 * 1024;

pub async fn start_mixed_meter_proxy(
    node_name: String,
    listen_port: u16,
    http_upstream_port: u16,
    socks_upstream_port: u16,
    metrics: Arc<Mutex<PinMetrics>>,
) -> Result<JoinHandle<()>> {
    let listener = TcpListener::bind(("127.0.0.1", listen_port))
        .await
        .with_context(|| format!("无法监听计量端口 {listen_port}"))?;

    Ok(tokio::spawn(async move {
        if let Err(err) = accept_loop(
            listener,
            node_name.clone(),
            http_upstream_port,
            socks_upstream_port,
            metrics,
        )
        .await
        {
            eprintln!("混合计量代理 {node_name} 已停止：{err:#}");
        }
    }))
}

async fn accept_loop(
    listener: TcpListener,
    node_name: String,
    http_upstream_port: u16,
    socks_upstream_port: u16,
    metrics: Arc<Mutex<PinMetrics>>,
) -> Result<()> {
    loop {
        let (client, _) = listener.accept().await?;
        let metrics = metrics.clone();
        let node_name = node_name.clone();
        tokio::spawn(async move {
            let result = match detect_protocol(&client).await {
                Ok(DetectedProtocol::Socks5) => {
                    handle_socks5_client(client, socks_upstream_port, metrics).await
                }
                Ok(DetectedProtocol::Http) => {
                    handle_http_client(client, http_upstream_port, metrics).await
                }
                Err(err) => Err(err),
            };
            if let Err(err) = result {
                eprintln!("mixed meter proxy for {node_name} failed: {err:#}");
            }
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectedProtocol {
    Http,
    Socks5,
}

async fn detect_protocol(client: &TcpStream) -> Result<DetectedProtocol> {
    let mut first = [0_u8; 1];
    let bytes = client.peek(&mut first).await?;
    if bytes == 0 {
        bail!("客户端在发送代理请求前已断开");
    }
    Ok(protocol_from_first_byte(first[0]))
}

fn protocol_from_first_byte(byte: u8) -> DetectedProtocol {
    if byte == 0x05 {
        DetectedProtocol::Socks5
    } else {
        DetectedProtocol::Http
    }
}

async fn handle_http_client(
    mut client: TcpStream,
    upstream_port: u16,
    metrics: Arc<Mutex<PinMetrics>>,
) -> Result<()> {
    let mut first_packet = Vec::new();
    let request = read_http_initial_request(&mut client, &mut first_packet).await?;
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

    relay_streams(client, upstream, metrics, connection_id).await
}

async fn handle_socks5_client(
    mut client: TcpStream,
    upstream_port: u16,
    metrics: Arc<Mutex<PinMetrics>>,
) -> Result<()> {
    let mut upstream = TcpStream::connect(("127.0.0.1", upstream_port))
        .await
        .with_context(|| format!("无法连接 mihomo SOCKS 上游端口 {upstream_port}"))?;

    let greeting = read_socks5_greeting(&mut client).await?;
    upstream.write_all(&greeting).await?;
    let greeting_reply = read_socks5_greeting_reply(&mut upstream).await?;
    client.write_all(&greeting_reply).await?;

    let request = read_socks5_connect_request(&mut client).await?;
    upstream.write_all(&request.raw).await?;
    let connect_reply = read_socks5_reply_like(&mut upstream).await?;
    client.write_all(&connect_reply).await?;

    let connection_id = {
        let mut guard = metrics.lock().map_err(|_| anyhow!("计量状态锁已损坏"))?;
        let id = guard.start_connection(request.target, "SOCKS5".to_string());
        guard.add_bytes(
            &id,
            TrafficDirection::Upload,
            (greeting.len() + request.raw.len()) as u64,
        );
        guard.add_bytes(
            &id,
            TrafficDirection::Download,
            (greeting_reply.len() + connect_reply.len()) as u64,
        );
        id
    };

    relay_streams(client, upstream, metrics, connection_id).await
}

async fn relay_streams(
    client: TcpStream,
    upstream: TcpStream,
    metrics: Arc<Mutex<PinMetrics>>,
    connection_id: String,
) -> Result<()> {
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
    metrics: Arc<Mutex<PinMetrics>>,
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

async fn read_http_initial_request(
    client: &mut TcpStream,
    buffer: &mut Vec<u8>,
) -> Result<InitialRequest> {
    let mut chunk = [0_u8; 4096];
    loop {
        let bytes = client.read(&mut chunk).await?;
        if bytes == 0 {
            return Err(anyhow!("客户端在发送代理请求前已断开"));
        }
        buffer.extend_from_slice(&chunk[..bytes]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            return parse_http_initial_request(buffer);
        }
        if buffer.len() > HEADER_LIMIT {
            return Err(anyhow!("HTTP 代理请求头超过 64KiB"));
        }
    }
}

fn parse_http_initial_request(buffer: &[u8]) -> Result<InitialRequest> {
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

async fn read_socks5_greeting(client: &mut TcpStream) -> Result<Vec<u8>> {
    let mut header = [0_u8; 2];
    client.read_exact(&mut header).await?;
    if header[0] != 0x05 {
        bail!("不支持的 SOCKS 版本：{}", header[0]);
    }
    let methods_len = header[1] as usize;
    let mut methods = vec![0_u8; methods_len];
    client.read_exact(&mut methods).await?;
    let mut raw = header.to_vec();
    raw.extend_from_slice(&methods);
    Ok(raw)
}

async fn read_socks5_greeting_reply(upstream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut reply = [0_u8; 2];
    upstream.read_exact(&mut reply).await?;
    if reply[0] != 0x05 {
        bail!("不支持的 SOCKS 响应版本：{}", reply[0]);
    }
    if reply[1] == 0xff {
        bail!("SOCKS5 上游没有可用认证方式");
    }
    Ok(reply.to_vec())
}

struct Socks5Request {
    raw: Vec<u8>,
    target: String,
}

async fn read_socks5_connect_request(client: &mut TcpStream) -> Result<Socks5Request> {
    let mut header = [0_u8; 4];
    client.read_exact(&mut header).await?;
    if header[0] != 0x05 {
        bail!("不支持的 SOCKS 请求版本：{}", header[0]);
    }
    if header[1] != 0x01 {
        bail!("SOCKS5 第一版只支持 TCP CONNECT");
    }

    let mut raw = header.to_vec();
    let host = match header[3] {
        0x01 => {
            let mut bytes = [0_u8; 4];
            client.read_exact(&mut bytes).await?;
            raw.extend_from_slice(&bytes);
            Ipv4Addr::from(bytes).to_string()
        }
        0x03 => {
            let mut len = [0_u8; 1];
            client.read_exact(&mut len).await?;
            raw.extend_from_slice(&len);
            let mut bytes = vec![0_u8; len[0] as usize];
            client.read_exact(&mut bytes).await?;
            raw.extend_from_slice(&bytes);
            String::from_utf8_lossy(&bytes).to_string()
        }
        0x04 => {
            let mut bytes = [0_u8; 16];
            client.read_exact(&mut bytes).await?;
            raw.extend_from_slice(&bytes);
            Ipv6Addr::from(bytes).to_string()
        }
        atyp => bail!("不支持的 SOCKS5 地址类型：{atyp}"),
    };

    let mut port_bytes = [0_u8; 2];
    client.read_exact(&mut port_bytes).await?;
    raw.extend_from_slice(&port_bytes);
    let port = u16::from_be_bytes(port_bytes);

    Ok(Socks5Request {
        raw,
        target: format!("{host}:{port}"),
    })
}

async fn read_socks5_reply_like(upstream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    upstream.read_exact(&mut header).await?;
    let mut raw = header.to_vec();
    match header[3] {
        0x01 => {
            let mut bytes = [0_u8; 4];
            upstream.read_exact(&mut bytes).await?;
            raw.extend_from_slice(&bytes);
        }
        0x03 => {
            let mut len = [0_u8; 1];
            upstream.read_exact(&mut len).await?;
            raw.extend_from_slice(&len);
            let mut bytes = vec![0_u8; len[0] as usize];
            upstream.read_exact(&mut bytes).await?;
            raw.extend_from_slice(&bytes);
        }
        0x04 => {
            let mut bytes = [0_u8; 16];
            upstream.read_exact(&mut bytes).await?;
            raw.extend_from_slice(&bytes);
        }
        atyp => bail!("不支持的 SOCKS5 响应地址类型：{atyp}"),
    }
    let mut port = [0_u8; 2];
    upstream.read_exact(&mut port).await?;
    raw.extend_from_slice(&port);
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connect_target() {
        let parsed = parse_http_initial_request(
            b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n",
        )
        .unwrap();
        assert_eq!(parsed.method, "CONNECT");
        assert_eq!(parsed.target, "example.com:443");
    }

    #[test]
    fn parses_http_url() {
        let parsed = parse_http_initial_request(
            b"GET http://example.com/path HTTP/1.1\r\nHost: example.com\r\n\r\n",
        )
        .unwrap();
        assert_eq!(parsed.method, "GET");
        assert_eq!(parsed.target, "http://example.com/path");
    }

    #[test]
    fn parses_socks5_domain_target() {
        let request = parse_socks5_connect_target(&[
            0x05, 0x01, 0x00, 0x03, 0x0b, b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'c',
            b'o', b'm', 0x01, 0xbb,
        ])
        .unwrap();
        assert_eq!(request, "example.com:443");
    }

    #[test]
    fn parses_socks5_ipv4_target() {
        let request =
            parse_socks5_connect_target(&[0x05, 0x01, 0x00, 0x01, 127, 0, 0, 1, 0x1f, 0x90])
                .unwrap();
        assert_eq!(request, "127.0.0.1:8080");
    }

    #[test]
    fn parses_socks5_ipv6_target() {
        let request = parse_socks5_connect_target(&[
            0x05, 0x01, 0x00, 0x04, 0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0,
            80,
        ])
        .unwrap();
        assert_eq!(request, "2001:db8::1:80");
    }

    #[test]
    fn detects_mixed_proxy_protocol() {
        assert_eq!(protocol_from_first_byte(0x05), DetectedProtocol::Socks5);
        assert_eq!(protocol_from_first_byte(b'C'), DetectedProtocol::Http);
        assert_eq!(protocol_from_first_byte(b'G'), DetectedProtocol::Http);
    }

    fn parse_socks5_connect_target(raw: &[u8]) -> Result<String> {
        if raw.len() < 7 || raw[0] != 0x05 || raw[1] != 0x01 {
            bail!("invalid socks request");
        }
        let mut index = 4;
        let host = match raw[3] {
            0x01 => {
                let bytes: [u8; 4] = raw[index..index + 4].try_into()?;
                index += 4;
                Ipv4Addr::from(bytes).to_string()
            }
            0x03 => {
                let len = raw[index] as usize;
                index += 1;
                let host = String::from_utf8_lossy(&raw[index..index + len]).to_string();
                index += len;
                host
            }
            0x04 => {
                let bytes: [u8; 16] = raw[index..index + 16].try_into()?;
                index += 16;
                Ipv6Addr::from(bytes).to_string()
            }
            _ => bail!("invalid atyp"),
        };
        let port = u16::from_be_bytes(raw[index..index + 2].try_into()?);
        Ok(format!("{host}:{port}"))
    }
}
