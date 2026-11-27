// request_handler routes by downcasting to the struct parse() returns. These
// ids used to have a second, never-parsed definition that the handler routed
// on, so Exit and char delete reservation silently fell through. Pin the struct
// each supported packetver resolves to.
use packets::packets::{
    Packet, PacketChDeleteCharReserved, PacketCzReqDisconnect, PacketCzReqName, PacketCzRequestMove, PacketCzRequestTime, PacketCzUseItem,
    PacketCzUseSkill, PacketHcDeleteCharReserved, PacketBiPing, PacketZcAckReqDisconnect, PacketZcAid,
};
use packets::packets_parser::{packet_len, parse};

const SUPPORTED_PACKETVERS: [u32; 3] = [20111102, 20120229, 20120307];

fn raw_bytes<P: Packet>(mut packet: P) -> Vec<u8> {
    packet.fill_raw();
    packet.raw().clone()
}

fn assert_parses_as<T: Packet + 'static>(bytes: &[u8], packetver: u32) {
    let parsed = parse(bytes, packetver);
    assert!(
        parsed.as_any().downcast_ref::<T>().is_some(),
        "packetver {packetver}: id {:02x}{:02x} parsed as {}, expected {}",
        bytes[1],
        bytes[0],
        parsed.name(),
        std::any::type_name::<T>()
    );
    assert_eq!(
        parsed.raw().len(),
        bytes.len(),
        "packetver {packetver}: {} consumed a wrong length",
        parsed.name()
    );
    assert_eq!(
        packet_len([bytes[0], bytes[1]], packetver),
        Some(bytes.len()),
        "packetver {packetver}: packet_len disagrees with {}",
        parsed.name()
    );
}

#[test]
fn routed_packets_resolve_to_the_struct_the_handler_downcasts_to() {
    for packetver in SUPPORTED_PACKETVERS {
        assert_parses_as::<PacketCzReqDisconnect>(&raw_bytes(PacketCzReqDisconnect::new(packetver)), packetver);
        assert_parses_as::<PacketZcAckReqDisconnect>(&raw_bytes(PacketZcAckReqDisconnect::new(packetver)), packetver);
        assert_parses_as::<PacketChDeleteCharReserved>(&raw_bytes(PacketChDeleteCharReserved::new(packetver)), packetver);
        assert_parses_as::<PacketHcDeleteCharReserved>(&raw_bytes(PacketHcDeleteCharReserved::new(packetver)), packetver);
        assert_parses_as::<PacketZcAid>(&raw_bytes(PacketZcAid::new(packetver)), packetver);
        assert_parses_as::<PacketBiPing>(&raw_bytes(PacketBiPing::new(packetver)), packetver);
    }
}

#[test]
fn removed_alias_ids_still_resolve_to_a_known_struct() {
    // Byte layouts the old aliases claimed; an id falling through to PacketUnknown
    // would swallow the rest of the read buffer.
    assert_parses_as::<PacketCzRequestMove>(&[0x5F, 0x03, 0x27, 0x89, 0xE0], 20111102);
    assert_parses_as::<PacketCzRequestTime>(&[0x60, 0x03, 0x01, 0x00, 0x00, 0x00], 20111102);
    assert_parses_as::<PacketCzReqName>(&[0x68, 0x03, 0x01, 0x00, 0x00, 0x00], 20120229);
    assert_parses_as::<PacketCzUseSkill>(&[0x38, 0x04, 0x01, 0x00, 0x1C, 0x00, 0x01, 0x00, 0x00, 0x00], 20120229);
    assert_parses_as::<PacketCzUseItem>(&[0x39, 0x04, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00], 20120307);
    assert_parses_as::<PacketBiPing>(&[0x87, 0x01, 0x01, 0x00, 0x00, 0x00], 20120307);
}

#[test]
fn ping_followed_by_request_time_splits_on_the_ping_length() {
    let packetver = 20120229;
    let mut stream = raw_bytes(PacketBiPing::new(packetver));
    let ping_len = stream.len();
    stream.extend(raw_bytes(PacketCzRequestTime::new(packetver)));

    let first = parse(&stream, packetver);
    assert!(first.as_any().downcast_ref::<PacketBiPing>().is_some());
    assert_eq!(first.raw().len(), ping_len);
    let second = parse(&stream[ping_len..], packetver);
    assert!(second.as_any().downcast_ref::<PacketCzRequestTime>().is_some());
}
