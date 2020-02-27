use std::{
    io::{self, Error, ErrorKind},
    mem,
    net::{SocketAddr, SocketAddrV4, SocketAddrV6, TcpStream},
    io::prelude::*,
    time::Duration,
    os::unix::io::{AsRawFd, RawFd},
    os::unix::net::UnixStream,
};

use net2::*;
use sendfd::*;
use tokio::net::UdpSocket;

/// Convert `sockaddr_storage` to `SocketAddr`
#[allow(dead_code)]
pub fn sockaddr_to_std(saddr: &libc::sockaddr_storage) -> io::Result<SocketAddr> {
    match saddr.ss_family as libc::c_int {
        libc::AF_INET => unsafe {
            let addr: SocketAddrV4 = mem::transmute_copy(saddr);
            Ok(SocketAddr::V4(addr))
        },
        libc::AF_INET6 => unsafe {
            let addr: SocketAddrV6 = mem::transmute_copy(saddr);
            Ok(SocketAddr::V6(addr))
        },
        _ => {
            let err = Error::new(ErrorKind::InvalidData, "family must be either AF_INET or AF_INET6");
            Err(err)
        }
    }
}

#[cfg(target_os="android")]
pub fn protect(protect_path: &Option<String>, fd: RawFd) -> io::Result<()> {
    // ignore if protect_path is not specified
    let path = match protect_path {
        Some(path) => path,
        None => return Ok(()),
    };

    // it's safe to use blocking socket here
    let mut stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::new(1, 0))).expect("couldn't set read timeout");
    stream.set_write_timeout(Some(Duration::new(1, 0))).expect("couldn't set write timeout");

    // send fds
    let dummy: [u8; 1] = [1];
    let fds: [RawFd; 1] = [fd];
    stream.send_with_fd(&dummy, &fds)?;

    // receive the return value
    let mut response = [0; 1];
    stream.read(&mut response)?;

    if response[0] == 0xFF {
        return Err(Error::new(ErrorKind::Other, "protect() failed"));
    }

    Ok(())
}

#[cfg(not(target_os="android"))]
pub fn protect(protect_path: &Option<String>, fd: RawFd) -> io::Result<()> {
    Ok(())
}

// create a new TCP stream
pub fn new_tcp_stream(protect_path: &Option<String>, saddr: &SocketAddr) -> io::Result<TcpStream> {
    let builder = match saddr {
        SocketAddr::V4(_) => TcpBuilder::new_v4()?,
        SocketAddr::V6(_) => TcpBuilder::new_v6()?,
    };

    protect(protect_path, builder.as_raw_fd())?;

    builder.to_tcp_stream()
}

/// Create a `UdpSocket` binded to `addr`
#[inline(always)]
pub async fn create_udp_socket(addr: &SocketAddr) -> io::Result<UdpSocket> {
    UdpSocket::bind(addr).await
}

/// Create a `UdpSocket` binded to `addr`
#[inline(always)]
pub async fn create_protected_udp_socket(protect_path: &Option<String>, addr: &SocketAddr) -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind(addr).await?;
    protect(protect_path, socket.as_raw_fd())?;
    Ok(socket)
}

