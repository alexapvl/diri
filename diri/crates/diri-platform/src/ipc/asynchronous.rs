//! IOCP registration for an already-connected Windows AF_UNIX stream.
//! Tokio's Windows TCP stream wrapper uses the same Winsock byte-stream I/O;
//! no TCP address, network listener or TCP-specific option is used here.
use std::os::windows::io::{FromRawSocket, IntoRawSocket};
use std::{
    io,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
pub use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

#[derive(Debug)]
pub struct UnixStream(tokio::net::TcpStream);
impl UnixStream {
    pub async fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_owned();
        // Local AF_UNIX connect may block on listen admission. Keep it off the
        // executor; the surrounding client handshake owns the operation timeout.
        let stream = tokio::task::spawn_blocking(move || super::UnixStream::connect(path))
            .await
            .map_err(io::Error::other)??;
        Self::from_std(stream)
    }
    pub fn from_std(stream: super::UnixStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        // SAFETY: ownership of one connected Winsock SOCK_STREAM transfers once.
        let stream = unsafe { std::net::TcpStream::from_raw_socket(stream.into_raw_socket()) };
        tokio::net::TcpStream::from_std(stream).map(Self)
    }
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        self.0.into_split()
    }
    pub fn pair() -> io::Result<(Self, Self)> {
        let (a, b) = super::UnixStream::pair()?;
        Ok((Self::from_std(a)?, Self::from_std(b)?))
    }
}
impl AsyncRead for UnixStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}
impl AsyncWrite for UnixStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}
