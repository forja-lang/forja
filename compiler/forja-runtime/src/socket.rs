//! Estado de sockets compartido entre VM y nativos.

use std::sync::{Arc, Mutex};

/// Tipo de socket representado internamente
#[derive(Debug, Clone, PartialEq)]
pub enum SocketTipo {
    TcpStream,
    TcpListener,
    UdpSocket,
}

/// Estado completo de un socket en el socket_heap de la VM
#[derive(Debug, Clone)]
pub struct SocketState {
    pub kind: SocketTipo,
    pub timeout_ms: Option<u64>,
    pub nonblocking: bool,
    pub connected: bool,
    pub local_addr: Option<String>,
    pub peer_addr: Option<String>,
    pub tcp_stream: Option<Arc<Mutex<std::net::TcpStream>>>,
    pub tcp_listener: Option<Arc<Mutex<std::net::TcpListener>>>,
    pub udp_socket: Option<Arc<Mutex<std::net::UdpSocket>>>,
}

impl SocketState {
    pub fn nuevo_tcp_stream(stream: std::net::TcpStream) -> Self {
        let local = stream.local_addr().ok().map(|a| a.to_string());
        let peer = stream.peer_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketTipo::TcpStream,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: peer,
            tcp_stream: Some(Arc::new(Mutex::new(stream))),
            tcp_listener: None,
            udp_socket: None,
        }
    }

    pub fn nuevo_tcp_listener(listener: std::net::TcpListener) -> Self {
        let local = listener.local_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketTipo::TcpListener,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: None,
            tcp_stream: None,
            tcp_listener: Some(Arc::new(Mutex::new(listener))),
            udp_socket: None,
        }
    }

    pub fn nuevo_udp_socket(socket: std::net::UdpSocket) -> Self {
        let local = socket.local_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketTipo::UdpSocket,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: None,
            tcp_stream: None,
            tcp_listener: None,
            udp_socket: Some(Arc::new(Mutex::new(socket))),
        }
    }

    pub fn cerrar(&mut self) {
        self.connected = false;
        if let Some(stream) = &self.tcp_stream {
            let _ = stream.lock().unwrap().shutdown(std::net::Shutdown::Both);
        }
        self.tcp_stream = None;
        self.tcp_listener = None;
        self.udp_socket = None;
    }
}
