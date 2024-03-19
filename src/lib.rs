//! Library for checking if there is already an instance of your application running,
//! and optionally sending an existing instance a message.
//!
//! This library deliberately aims to be simple and lightweight, so it **only supports
//! a single existing instance**.

#![warn(missing_docs)]

use {
    interprocess::local_socket::{LocalSocketListener, LocalSocketStream},
    std::io::{Read, Write},
};

use std::io::ErrorKind;

/// Communication endpoint between an exsiting and a new instance
pub enum Endpoint {
    /// You are the new instance
    New(Listener),
    /// There is already an existing instance running
    Existing(Stream),
}

/// IPC listener to listen to incoming connections
pub struct Listener(LocalSocketListener);

impl Listener {
    /// Accept an incoming connection.
    ///
    /// If you don't need to send or receive data, you can just check `accept.is_some()`.
    /// This is sufficient if you just want to do something like focus a window, if there
    /// was an attempted connection by a new instance.
    pub fn accept(&self) -> Option<Stream> {
        match self.0.accept() {
            Ok(stream) => Some(Stream(stream)),
            Err(e) => {
                log::error!("{e:?}");
                None
            }
        }
    }
}

/// Message between two processes
#[derive(Debug)]
#[repr(u8)]
pub enum Msg {
    /// A number
    Num(usize) = 0,
    /// Arbitrary byte data
    Bytes(Vec<u8>),
    /// UTF-8 string
    String(String),
}

fn write_u8(num: u8, stream: &mut LocalSocketStream) -> std::io::Result<()> {
    stream.write_all(std::slice::from_ref(&num))
}

fn read_u8(stream: &mut LocalSocketStream) -> std::io::Result<u8> {
    let mut num: u8 = 0;
    stream.read_exact(std::slice::from_mut(&mut num))?;
    Ok(num)
}

fn write_usize(num: usize, stream: &mut LocalSocketStream) -> std::io::Result<()> {
    let bytes = num.to_le_bytes();
    stream.write_all(&bytes)
}

fn read_usize(stream: &mut LocalSocketStream) -> std::io::Result<usize> {
    let mut buf = [0; std::mem::size_of::<usize>()];
    stream.read_exact(&mut buf)?;
    Ok(usize::from_le_bytes(buf))
}

fn read_vec(stream: &mut LocalSocketStream) -> std::io::Result<Vec<u8>> {
    let len = read_usize(stream)?;
    log::debug!("read_vec: length: {len}");
    let mut buf = vec![0; len];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}

impl Msg {
    fn discriminant(&self) -> u8 {
        unsafe { *(self as *const Self as *const u8) }
    }
    fn write(self, stream: &mut LocalSocketStream) {
        let discriminant = self.discriminant();
        log::debug!("Writing discriminant {discriminant}");
        write_u8(discriminant, stream).unwrap();
        match self {
            Msg::Num(n) => {
                write_usize(n, stream).unwrap();
            }
            Msg::Bytes(bytes) => {
                write_usize(bytes.len(), stream).unwrap();
                log::debug!("Wrote byte length: {}", bytes.len());
                stream.write_all(&bytes).unwrap();
            }
            Msg::String(str) => {
                write_usize(str.len(), stream).unwrap();
                log::debug!("Wrote byte length: {}", str.len());
                stream.write_all(str.as_bytes()).unwrap();
            }
        }
    }
    fn read(stream: &mut LocalSocketStream) -> std::io::Result<Self> {
        let discriminant = read_u8(stream)?;
        log::debug!("Read discriminant {discriminant}");
        match discriminant {
            0 => Ok(Self::Num(read_usize(stream)?)),
            1 => Ok(Self::Bytes(read_vec(stream)?)),
            2 => {
                log::debug!("Reading string...");
                let bytes = read_vec(stream)?;
                Ok(Self::String(String::from_utf8_lossy(&bytes).into_owned()))
            }
            etc => panic!("Unknown message discriminant {etc}"),
        }
    }
}

/// IPC message stream with a simple protocol
pub struct Stream(LocalSocketStream);

impl Stream {
    /// Send a message to the recipient
    pub fn send(&mut self, msg: Msg) {
        msg.write(&mut self.0)
    }
    /// Receive a message, if any
    pub fn recv(&mut self) -> Option<Msg> {
        match Msg::read(&mut self.0) {
            Ok(msg) => Some(msg),
            Err(e) => {
                log::error!("Stream::recv error: {e}");
                None
            }
        }
    }
}

/// Connect to an existing instance, or establish self as the existing instance
///
/// The id should be a string unique to your application that's valid as a file name.
pub fn establish_endpoint(id: &str, nonblocking: bool) -> std::io::Result<Endpoint> {
    // Using interprocess crate's namespace syntax
    let namespace_name = format!("@{id}");
    match LocalSocketStream::connect(&namespace_name[..]) {
        Ok(stream) => Ok(Endpoint::Existing(Stream(stream))),
        Err(e) => match e.kind() {
            ErrorKind::NotFound | ErrorKind::ConnectionRefused => {
                let socket = LocalSocketListener::bind(&namespace_name[..])?;
                socket.set_nonblocking(nonblocking)?;
                log::info!("Established new endpoint with name {namespace_name}");
                Ok(Endpoint::New(Listener(socket)))
            }
            _ => Err(e),
        },
    }
}
