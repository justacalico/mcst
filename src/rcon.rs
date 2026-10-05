//! Source RCON protocol client — used for servers where mcst does not own
//! stdin (imported/custom servers with enable-rcon=true).

use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const TYPE_AUTH: i32 = 3;
const TYPE_COMMAND: i32 = 2;
const TYPE_RESPONSE: i32 = 0;
const ID_AUTH_FAILED: i32 = -1;

pub struct Rcon {
    stream: TcpStream,
    counter: i32,
}

fn encode(id: i32, ty: i32, payload: &[u8]) -> Vec<u8> {
    let len = 4 + 4 + payload.len() + 2;
    let mut out = Vec::with_capacity(len + 4);
    out.extend_from_slice(&(len as i32).to_le_bytes());
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&ty.to_le_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(&[0, 0]);
    out
}

async fn read_packet(s: &mut TcpStream) -> Result<(i32, i32, Vec<u8>)> {
    let mut len_buf = [0u8; 4];
    s.read_exact(&mut len_buf).await?;
    let len = i32::from_le_bytes(len_buf) as usize;
    if !(10..=1_048_576).contains(&len) {
        bail!("bad rcon packet length {len}");
    }
    let mut buf = vec![0u8; len];
    s.read_exact(&mut buf).await?;
    let id = i32::from_le_bytes(buf[0..4].try_into().unwrap());
    let ty = i32::from_le_bytes(buf[4..8].try_into().unwrap());
    let payload = buf[8..len - 2].to_vec();
    Ok((id, ty, payload))
}

impl Rcon {
    /// Connect and authenticate.
    pub async fn connect(addr: &str, password: &str) -> Result<Self> {
        let mut stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(addr))
            .await
            .context("rcon connect timed out")??;
        stream
            .write_all(&encode(1, TYPE_AUTH, password.as_bytes()))
            .await?;
        let (id, _ty, _p) =
            tokio::time::timeout(Duration::from_secs(5), read_packet(&mut stream)).await??;
        if id == ID_AUTH_FAILED {
            bail!("rcon authentication failed");
        }
        Ok(Self { stream, counter: 2 })
    }

    /// Run a command, returning the server's response text.
    pub async fn command(&mut self, cmd: &str) -> Result<String> {
        let id = self.counter;
        self.counter += 1;
        self.stream
            .write_all(&encode(id, TYPE_COMMAND, cmd.as_bytes()))
            .await?;
        let (rid, ty, payload) =
            tokio::time::timeout(Duration::from_secs(10), read_packet(&mut self.stream)).await??;
        if rid != id {
            bail!("rcon id mismatch");
        }
        // Multi-packet responses append with the same id; type 0 is the
        // response payload type.
        debug_assert_eq!(ty, TYPE_RESPONSE);
        Ok(String::from_utf8_lossy(&payload).to_string())
    }
}

/// One-shot helper: connect, run a command, disconnect.
pub async fn run(addr: &str, password: &str, cmd: &str) -> Result<String> {
    let mut r = Rcon::connect(addr, password).await?;
    r.command(cmd).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn encode_framing() {
        let p = encode(7, TYPE_COMMAND, b"list");
        let len = i32::from_le_bytes(p[0..4].try_into().unwrap()) as usize;
        assert_eq!(len, p.len() - 4);
        assert_eq!(i32::from_le_bytes(p[4..8].try_into().unwrap()), 7);
        assert_eq!(
            i32::from_le_bytes(p[8..12].try_into().unwrap()),
            TYPE_COMMAND
        );
        assert_eq!(&p[12..16], b"list");
        assert_eq!(&p[p.len() - 2..], &[0, 0]);
    }

    /// A fake RCON server that accepts any password and echoes commands.
    async fn fake_server() -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let h = tokio::spawn(async move {
            let (mut c, _) = listener.accept().await.unwrap();
            // auth
            let (_id, ty, _p) = read_packet(&mut c).await.unwrap();
            assert_eq!(ty, TYPE_AUTH);
            let resp = encode(1, TYPE_RESPONSE, b"");
            c.write_all(&resp).await.unwrap();
            // commands
            while let Ok((id, ty, p)) = read_packet(&mut c).await {
                if ty != TYPE_COMMAND {
                    break;
                }
                let echo = format!("ok: {}", String::from_utf8_lossy(&p));
                c.write_all(&encode(id, TYPE_RESPONSE, echo.as_bytes()))
                    .await
                    .unwrap();
            }
        });
        (port, h)
    }

    #[tokio::test]
    async fn connect_and_command() {
        let (port, _h) = fake_server().await;
        let mut r = Rcon::connect(&format!("127.0.0.1:{port}"), "pw")
            .await
            .unwrap();
        let out = r.command("list").await.unwrap();
        assert_eq!(out, "ok: list");
        let out2 = r.command("say hi").await.unwrap();
        assert_eq!(out2, "ok: say hi");
    }

    #[tokio::test]
    async fn bad_password_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut c, _) = listener.accept().await.unwrap();
            let _ = read_packet(&mut c).await.unwrap();
            c.write_all(&encode(ID_AUTH_FAILED, TYPE_RESPONSE, b""))
                .await
                .unwrap();
        });
        assert!(Rcon::connect(&format!("127.0.0.1:{port}"), "pw")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn run_helper() {
        let (port, _h) = fake_server().await;
        let out = run(&format!("127.0.0.1:{port}"), "pw", "tps")
            .await
            .unwrap();
        assert_eq!(out, "ok: tps");
    }
}
