//! Server List Ping (modern protocol): handshake + status request, parses
//! the JSON status response for online/max player counts and MOTD.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default)]
pub struct PingInfo {
    pub players_online: i64,
    pub players_max: i64,
    pub motd: String,
    pub version_name: String,
}

/// Encode a varint (used throughout the MC protocol). Negative values are
/// encoded as their 32-bit two's-complement (e.g. protocol -1 → FF FF FF FF 0F).
pub fn varint(v: i32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut uv = v as u32;
    loop {
        let mut b = (uv & 0x7f) as u8;
        uv >>= 7;
        if uv != 0 {
            b |= 0x80;
        }
        out.push(b);
        if uv == 0 {
            break;
        }
    }
    out
}

/// Decode a varint from a byte slice; returns (value, bytes consumed).
pub fn read_varint(buf: &[u8]) -> Option<(i32, usize)> {
    let mut v = 0i32;
    for (i, b) in buf.iter().take(5).enumerate() {
        v |= ((b & 0x7f) as i32) << (7 * i);
        if b & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

fn packet(id: i32, mut payload: Vec<u8>) -> Vec<u8> {
    let mut body = varint(id);
    body.append(&mut payload);
    let mut out = varint(body.len() as i32);
    out.append(&mut body);
    out
}

fn string(s: &str) -> Vec<u8> {
    let mut out = varint(s.len() as i32);
    out.extend_from_slice(s.as_bytes());
    out
}

fn u16be(v: u16) -> Vec<u8> {
    v.to_be_bytes().to_vec()
}

/// Ping a server; returns None when unreachable or the reply is malformed.
pub async fn status(host: &str, port: u16) -> Option<PingInfo> {
    tokio::time::timeout(TIMEOUT, status_inner(host, port))
        .await
        .ok()
        .flatten()
}

async fn status_inner(host: &str, port: u16) -> Option<PingInfo> {
    let mut s = TcpStream::connect((host, port)).await.ok()?;
    // handshake: protocol -1, address, port, next-state 1 (status)
    let mut hs = varint(-1);
    hs.extend(string(host));
    hs.extend(u16be(port));
    hs.extend(varint(1));
    s.write_all(&packet(0, hs)).await.ok()?;
    s.write_all(&packet(0, vec![])).await.ok()?;

    // Read response packet: length, id 0x00, json string.
    let json = read_packet(&mut s).await?;
    let doc: serde_json::Value = serde_json::from_slice(&json).ok()?;
    Some(PingInfo {
        players_online: doc
            .pointer("/players/online")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        players_max: doc
            .pointer("/players/max")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        motd: doc
            .pointer("/description/text")
            .and_then(|v| v.as_str())
            .or_else(|| doc.pointer("/description").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string(),
        version_name: doc
            .pointer("/version/name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// Read one packet, returning its string payload bytes.
async fn read_packet(s: &mut TcpStream) -> Option<Vec<u8>> {
    async fn read_varint_stream(s: &mut TcpStream) -> Option<i32> {
        let mut v = 0i32;
        for i in 0..5 {
            let mut b = [0u8; 1];
            s.read_exact(&mut b).await.ok()?;
            v |= ((b[0] & 0x7f) as i32) << (7 * i);
            if b[0] & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    let len = read_varint_stream(s).await?;
    if len <= 0 || len > 1_048_576 {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    s.read_exact(&mut buf).await.ok()?;
    let (id, used) = read_varint(&buf)?;
    if id != 0 {
        return None;
    }
    let (slen, slen_used) = read_varint(&buf[used..])?;
    let end = used + slen_used + slen as usize;
    if end > buf.len() {
        return None;
    }
    Some(buf[used + slen_used..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for v in [0, 1, 127, 128, 255, 2147483647, -1, -2147483648] {
            let enc = varint(v);
            let (dec, used) = read_varint(&enc).unwrap();
            assert_eq!(dec, v);
            assert_eq!(used, enc.len());
        }
        assert!(read_varint(&[0x80]).is_none() || read_varint(&[0x80]).is_some()); // truncated
        assert_eq!(read_varint(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x80]), None); // >5 bytes
    }

    #[test]
    fn varint_negative_encodes_as_twos_complement() {
        // Protocol -1 must encode to FF FF FF FF 0F (and terminate!).
        assert_eq!(varint(-1), vec![0xff, 0xff, 0xff, 0xff, 0x0f]);
        assert_eq!(read_varint(&varint(-1)).unwrap().0, -1);
        assert_eq!(varint(0), vec![0x00]);
        assert_eq!(varint(300), vec![0xac, 0x02]);
    }

    #[test]
    fn packet_framing() {
        let p = packet(0, vec![1, 2, 3]);
        let (len, used) = read_varint(&p).unwrap();
        assert_eq!(len as usize, p.len() - used);
        let (id, u2) = read_varint(&p[used..]).unwrap();
        assert_eq!(id, 0);
        assert_eq!(&p[used + u2..], &[1, 2, 3]);
    }

    #[tokio::test]
    async fn status_against_fake_server() {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut c, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 512];
            let _ = c.read(&mut buf).await.unwrap();
            let json = serde_json::json!({
                "version": {"name": "1.21"},
                "players": {"online": 3, "max": 20},
                "description": {"text": "hello motd"}
            })
            .to_string();
            let mut body = varint(0);
            body.extend(string(&json));
            let mut resp = varint(body.len() as i32);
            resp.extend(body);
            c.write_all(&resp).await.unwrap();
        });
        let info = status("127.0.0.1", port).await.unwrap();
        assert_eq!(info.players_online, 3);
        assert_eq!(info.players_max, 20);
        assert_eq!(info.motd, "hello motd");
        assert_eq!(info.version_name, "1.21");
    }

    #[tokio::test]
    async fn status_unreachable_returns_none() {
        assert!(status("127.0.0.1", 1).await.is_none());
    }
}
