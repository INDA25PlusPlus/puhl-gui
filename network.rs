use std::{io::{Read, Write}, net::{TcpStream}};

use crate::protocol::{parse, serialize, Message, SerializeError, ParseError};

#[derive(Debug)]
pub enum NetError {
    ParseError(ParseError),
    SerializeError(SerializeError),
    IoError(std::io::Error),
}

impl From<ParseError> for NetError {
    fn from(e: ParseError) -> Self {
        NetError::ParseError(e)
    }
}

impl From<SerializeError> for NetError {
    fn from(e: SerializeError) -> Self {
        NetError::SerializeError(e)
    }
}

impl From<std::io::Error> for NetError {
    fn from(e: std::io::Error) -> Self {
        NetError::IoError(e)
    }
}

pub fn read_message(stream: &mut TcpStream) -> Result<Message, NetError> {
    let mut message = [0; 128];
    let buf = stream.read_exact(&mut message);
    match buf {
        Ok(_) => {
            let str = String::from_utf8_lossy(&message).to_string();
            let message = parse(&str)?;
            Ok(message)
        },
        Err(e) => Err(NetError::IoError(e))
    }
}

pub fn send_message(mut stream: &TcpStream, message: &Message) -> Result<(), NetError> {
    let message = serialize(message)?;
    stream.write_all(message.as_bytes())?;

    Ok(())
}