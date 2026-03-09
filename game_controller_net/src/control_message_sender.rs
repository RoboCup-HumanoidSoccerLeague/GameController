use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use anyhow::Result;
use bytes::Bytes;
use tokio::{net::UdpSocket, select, sync::watch, time::interval};

use game_controller_core::types::{Game, Params};
use game_controller_msgs::{ControlMessage, CONTROL_MESSAGE_PORT};

/// This struct represents a sender for control messages. The messages are sent as UDP packets to
/// the given destination address. The states to be sent are obtained from a [tokio::sync::watch]
/// channel. This struct works both for sending to players and sending to monitor applications. The
/// caller can select what is desired by supplying an appropriate destination address and the flag.
pub struct ControlMessageSender {
    socket: UdpSocket,
    params: Params,
    game_receiver: watch::Receiver<Game>,
    to_monitor: bool,
}

impl ControlMessageSender {
    /// The period at which control messages are sent.
    const SEND_INTERVAL: Duration = Duration::from_millis(500);

    /// The period at which control messages are sent after switching to the stopped state.
    const SEND_INTERVAL_FAST: Duration = Duration::from_millis(200);

    /// The number of messages that are sent at the fast interval before switching back to normal.
    const SEND_FAST_COUNTER: u8 = 5;

    /// This function creates a new sender for control messages.
    pub async fn new(
        address: IpAddr,
        params: Params,
        game_receiver: watch::Receiver<Game>,
        to_monitor: bool,
    ) -> Result<Self> {
        Ok(Self {
            socket: {
                let socket = UdpSocket::bind((
                    match address {
                        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                    },
                    0u16,
                ))
                .await?;
                socket.set_broadcast(true)?;
                socket.connect((address, CONTROL_MESSAGE_PORT)).await?;
                socket
            },
            params,
            game_receiver,
            to_monitor,
        })
    }

    /// This function runs the sender indefinitely.
    pub async fn run(&mut self) {
        let mut send_interval = interval(Self::SEND_INTERVAL);
        let mut packet_number: u8 = 0;
        let mut last_stopped: bool = false;
        let mut countdown: u8 = 0;
        loop {
            select! {
                _ = send_interval.tick() => {
                    if countdown > 0 {
                        countdown -= 1;
                        if countdown == 0 {
                            // We have sent enough "fast" messages. Continue because the interval
                            // will fire immediately.
                            send_interval = interval(Self::SEND_INTERVAL);
                            continue;
                        }
                    }
                },
                game = self.game_receiver.wait_for(|game| game.stopped != last_stopped) => {
                    if let Ok(game) = game {
                        last_stopped = game.stopped;
                    } else {
                        return;
                    }
                    if last_stopped {
                        countdown = Self::SEND_FAST_COUNTER;
                        send_interval = interval(Self::SEND_INTERVAL_FAST);
                    }
                    // Continue because either the interval will fire immediately (stopped = true)
                    // or it's okay to wait until the next regular tick.
                    continue;
                },
            };
            // It would be better if the timers were updated to the current time before
            // serialization. At the moment, this is not necessary because the main thread updates
            // the state each time a (rounded) timer crosses a second boundary. However, this tight
            // coupling between the fact that timers are sent as seconds and the main logic is
            // undesirable.
            let buffer: Bytes = ControlMessage::new(
                &self.game_receiver.borrow(),
                &self.params,
                packet_number,
                self.to_monitor,
            )
            .into();
            let _ = self.socket.send(&buffer).await;
            packet_number = packet_number.wrapping_add(1);
        }
    }
}
