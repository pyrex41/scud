//! Event publishing system using ZMQ.
//!
//! This module provides ZMQ-based event publishing for SCUD task management.
//! It supports publishing task events via PUB socket and handling requests via REP socket.

use anyhow::{anyhow, Result};
use zmq;

/// Event publisher for SCUD task management events.
///
/// Manages ZMQ sockets for publishing events and handling requests.
pub struct EventPublisher {
    /// ZMQ context (kept alive for socket lifetime)
    #[allow(dead_code)]
    context: zmq::Context,
    /// PUB socket for broadcasting task events
    pub_socket: zmq::Socket,
    /// REP socket for handling requests
    rep_socket: zmq::Socket,
}

impl EventPublisher {
    /// Create a new EventPublisher with ZMQ sockets.
    ///
    /// # Arguments
    /// * `pub_endpoint` - Endpoint for the PUB socket (e.g., "tcp://*:5555")
    /// * `rep_endpoint` - Endpoint for the REP socket (e.g., "tcp://*:5556")
    ///
    /// # Returns
    /// A Result containing the EventPublisher or an error
    pub fn new(pub_endpoint: &str, rep_endpoint: &str) -> Result<Self> {
        let context = zmq::Context::new();

        // Create PUB socket for publishing events
        let pub_socket = context
            .socket(zmq::PUB)
            .map_err(|e| anyhow!("Failed to create PUB socket: {}", e))?;

        // Create REP socket for handling requests
        let rep_socket = context
            .socket(zmq::REP)
            .map_err(|e| anyhow!("Failed to create REP socket: {}", e))?;

        // Bind sockets to endpoints
        pub_socket
            .bind(pub_endpoint)
            .map_err(|e| anyhow!("Failed to bind PUB socket to {}: {}", pub_endpoint, e))?;

        rep_socket
            .bind(rep_endpoint)
            .map_err(|e| anyhow!("Failed to bind REP socket to {}: {}", rep_endpoint, e))?;

        Ok(EventPublisher {
            context,
            pub_socket,
            rep_socket,
        })
    }

    /// Publish an event message.
    ///
    /// # Arguments
    /// * `topic` - The topic string for the message
    /// * `message` - The message content
    ///
    /// # Returns
    /// A Result indicating success or failure
    pub fn publish(&self, topic: &str, message: &str) -> Result<()> {
        let full_message = format!("{} {}", topic, message);
        self.pub_socket
            .send(&full_message, 0)
            .map_err(|e| anyhow!("Failed to send message: {}", e))?;
        Ok(())
    }

    /// Receive a request message.
    ///
    /// This is a blocking call that waits for a request.
    ///
    /// # Returns
    /// A Result containing the received message or an error
    pub fn receive_request(&self) -> Result<String> {
        let mut msg = zmq::Message::new();
        self.rep_socket
            .recv(&mut msg, 0)
            .map_err(|e| anyhow!("Failed to receive request: {}", e))?;
        Ok(msg.as_str().unwrap_or("").to_string())
    }

    /// Send a reply message.
    ///
    /// # Arguments
    /// * `reply` - The reply message to send
    ///
    /// # Returns
    /// A Result indicating success or failure
    pub fn send_reply(&self, reply: &str) -> Result<()> {
        self.rep_socket
            .send(reply, 0)
            .map_err(|e| anyhow!("Failed to send reply: {}", e))?;
        Ok(())
    }
}

impl Drop for EventPublisher {
    fn drop(&mut self) {
        // ZMQ sockets and context will be cleaned up automatically
    }
}
