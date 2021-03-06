/// Helper structs to build b2ghald clients.
use crate::messages::*;
use bincode::Options;
use log::error;
use std::collections::HashMap;
use std::io;
use std::io::{Error, ErrorKind};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, Sender};

pub enum HalError {
    StreamError,
    NoListener,
}

pub struct HalClient {
    stream: UnixStream,
    req_id: u64,
    listeners: HashMap<u64, Sender<Response>>,
}

impl HalClient {
    pub fn connect(path: &str) -> Result<Self, io::Error> {
        match UnixStream::connect(path) {
            Ok(stream) => Ok(Self {
                stream,
                req_id: 0,
                listeners: HashMap::new(),
            }),
            Err(err) => {
                error!("Failed to connect to b2ghald at {}: {}", path, err);
                Err(err)
            }
        }
    }

    pub fn send(&mut self, request: Request, sender: Sender<Response>) -> Result<(), io::Error> {
        let id = self.req_id;
        self.req_id += 1;
        let message = ToDaemon::new(id, request);
        self.listeners.insert(id, sender);

        let config = bincode::DefaultOptions::new().with_native_endian();

        config
            .serialize_into(&self.stream, &message)
            .map_err(|_| Error::new(ErrorKind::Other, "bincode error"))?;

        Ok(())
    }

    // Blocks to get the next message, and dispatch it to the receiver.
    pub fn get_next_message(&mut self) -> Result<(), HalError> {
        let config = bincode::DefaultOptions::new().with_native_endian();
        if let Ok(message) = config.deserialize_from::<_, FromDaemon>(&self.stream) {
            if let Some(listener) = self.listeners.remove(&message.id()) {
                let _ = listener.send((*message.response()).clone());
            } else {
                error!("No listener registered for message #{}", message.id());
                return Err(HalError::NoListener);
            }
        } else {
            error!("Failed to deserialize messages.");
            return Err(HalError::StreamError);
        }

        Ok(())
    }
}

// A simple, blocking client.
pub struct SimpleClient {
    client: HalClient,
}

impl SimpleClient {
    pub fn new() -> Option<Self> {
        match HalClient::connect("/tmp/b2ghald.sock") {
            Ok(client) => Some(Self { client }),
            Err(_) => None,
        }
    }

    pub fn set_screen_brightness(&mut self, value: u8) {
        let (sender, receiver) = channel();
        let _ = self.client.send(Request::SetBrightness(value), sender);
        if self.client.get_next_message().is_ok() {
            let _ = receiver.recv();
        }
    }

    pub fn get_screen_brightness(&mut self) -> u8 {
        let (sender, receiver) = channel();
        let _ = self.client.send(Request::GetBrightness, sender);
        if self.client.get_next_message().is_ok() {
            match receiver.recv() {
                Ok(Response::GetBrightnessSuccess(value)) => value,
                Ok(_) | Err(_) => 0,
            }
        } else {
            0
        }
    }

    pub fn enable_screen(&mut self, screen_id: u8) {
        let (sender, receiver) = channel();
        let _ = self.client.send(Request::EnableScreen(screen_id), sender);
        if self.client.get_next_message().is_ok() {
            let _ = receiver.recv();
        }
    }

    pub fn disable_screen(&mut self, screen_id: u8) {
        let (sender, receiver) = channel();
        let _ = self.client.send(Request::DisableScreen(screen_id), sender);
        if self.client.get_next_message().is_ok() {
            let _ = receiver.recv();
        }
    }

    pub fn reboot(&mut self) {
        let (sender, receiver) = channel();
        let _ = self.client.send(Request::Reboot, sender);
        if self.client.get_next_message().is_ok() {
            let _ = receiver.recv();
        }
    }

    pub fn poweroff(&mut self) {
        let (sender, receiver) = channel();
        let _ = self.client.send(Request::PowerOff, sender);
        if self.client.get_next_message().is_ok() {
            let _ = receiver.recv();
        }
    }
}
