use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::panic;

use packets::packets::{
    Packet, PacketZcAckReqNameAll, PacketZcAckReqNameAllWithTitle, PacketZcNotifyAct, PacketZcNotifyActShortDamage,
    PacketZcNotifyActSpDamage, PacketZcNotifyMapProperty, PacketZcNotifyMapPropertyWithFlags,
};
use packets::packets_parser::parse;

use crate::server::service::global_config_service::GlobalConfigService;
use crate::util::tick::get_tick;

pub fn chain_packets(packets: Vec<&dyn Packet>) -> Vec<u8> {
    let mut res: Vec<u8> = Vec::new();
    for packet in packets {
        res = res.iter().cloned().chain(packet.raw().iter().cloned()).collect();
    }
    res
}

#[allow(dead_code)]
pub fn chain_packets_raws(packets_raw: Vec<&Vec<u8>>) -> Vec<u8> {
    let mut res: Vec<u8> = Vec::new();
    for raw in packets_raw {
        res = res.iter().cloned().chain(raw.iter().cloned()).collect();
    }
    res
}

pub fn chain_packets_raws_by_value(packets_raw: Vec<Vec<u8>>) -> Vec<u8> {
    let mut res: Vec<u8> = Vec::new();
    for raw in packets_raw {
        res = res.iter().cloned().chain(raw).collect();
    }
    res
}

// Version cutoffs follow rAthena packets.hpp / packets_struct.hpp / clif.cpp
// (main client numbering).
const NOTIFY_ACT_LONG_DAMAGE_SINCE: u32 = 20071113;
const NOTIFY_ACT_SP_DAMAGE_SINCE: u32 = 20131223;
const ACK_REQ_NAME_ALL_WITH_TITLE_SINCE: u32 = 20150225;
const NOTIFY_MAP_PROPERTY_WITH_FLAGS_SINCE: u32 = 20121010;

/// rAthena `enum map_property`: what the client shows for the map (pvp,
/// woe...).
pub const MAP_PROPERTY_NOTHING: i16 = 0;

#[derive(Debug, Default, Clone, Copy)]
pub struct NotifyAct {
    pub gid: u32,
    pub target_gid: u32,
    pub attack_mt: i32,
    pub attacked_mt: i32,
    pub damage: i32,
    pub count: i16,
    pub action: u8,
}

/// ZC_NOTIFY_ACT: 0x008a before 20071113 (short damage), 0x02e1 until 20131223,
/// 0x08c8 after.
pub fn build_notify_act(packetver: u32, act: NotifyAct) -> Vec<u8> {
    if packetver < NOTIFY_ACT_LONG_DAMAGE_SINCE {
        let mut packet = PacketZcNotifyActShortDamage::new(packetver);
        packet.set_gid(act.gid);
        packet.set_target_gid(act.target_gid);
        packet.set_attack_mt(act.attack_mt);
        packet.set_attacked_mt(act.attacked_mt);
        packet.set_damage(act.damage.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
        packet.set_count(act.count);
        packet.set_action(act.action);
        packet.fill_raw();
        packet.raw
    } else if packetver < NOTIFY_ACT_SP_DAMAGE_SINCE {
        let mut packet = PacketZcNotifyAct::new(packetver);
        packet.set_gid(act.gid);
        packet.set_target_gid(act.target_gid);
        packet.set_attack_mt(act.attack_mt);
        packet.set_attacked_mt(act.attacked_mt);
        packet.set_damage(act.damage);
        packet.set_count(act.count);
        packet.set_action(act.action);
        packet.fill_raw();
        packet.raw
    } else {
        let mut packet = PacketZcNotifyActSpDamage::new(packetver);
        packet.set_gid(act.gid);
        packet.set_target_gid(act.target_gid);
        packet.set_attack_mt(act.attack_mt);
        packet.set_attacked_mt(act.attacked_mt);
        packet.set_damage(act.damage);
        packet.set_is_sp_damaged(false);
        packet.set_count(act.count);
        packet.set_action(act.action);
        packet.fill_raw();
        packet.raw
    }
}

/// ZC_ACK_REQNAMEALL: 0x0195 before 20150225, 0x0a30 (with title id) after.
pub fn build_ack_req_name_all(packetver: u32, gid: u32, name: [char; 24]) -> Vec<u8> {
    if packetver < ACK_REQ_NAME_ALL_WITH_TITLE_SINCE {
        let mut packet = PacketZcAckReqNameAll::new(packetver);
        packet.set_aid(gid);
        packet.set_cname(name);
        packet.fill_raw();
        packet.raw
    } else {
        let mut packet = PacketZcAckReqNameAllWithTitle::new(packetver);
        packet.set_gid(gid);
        packet.set_name(name);
        packet.fill_raw();
        packet.raw
    }
}

/// ZC_NOTIFY_MAPPROPERTY: 0x0199 (type only) before 20121010, 0x099b (type +
/// flags) after.
pub fn build_notify_map_property(packetver: u32, property: i16, flags: u32) -> Vec<u8> {
    if packetver < NOTIFY_MAP_PROPERTY_WITH_FLAGS_SINCE {
        let mut packet = PacketZcNotifyMapProperty::new(packetver);
        packet.set_atype(property);
        packet.fill_raw();
        packet.raw
    } else {
        let mut packet = PacketZcNotifyMapPropertyWithFlags::new(packetver);
        packet.set_atype(property);
        packet.set_flags(flags);
        packet.fill_raw();
        packet.raw
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PacketDirection {
    Forward,
    Backward,
}

impl Display for PacketDirection {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub fn debug_packets_from_vec(
    outgoing: Option<&SocketAddr>,
    direction: PacketDirection,
    packetver: u32,
    bytes: &Vec<u8>,
    name: &Option<String>,
) {
    let mut buffer = [0_u8; 20048];
    if bytes.len() < 2 {
        return;
    }
    if bytes.len() > buffer.len() {
        panic!("Bulk of packet is greater {} than buffer {}", bytes.len(), buffer.len());
    }
    for (i, byte) in bytes.iter().enumerate() {
        if i >= buffer.len() {
            break;
        }
        buffer[i] = *byte;
    }
    if bytes.len() >= buffer.len() {
        panic!(
            "Can't debug packet {:02X?}\n from bulk packets {:02X?}",
            buffer,
            bytes.as_slice()
        )
    }
    debug_packets(outgoing, direction, packetver, &buffer, bytes.len(), name);
}
pub fn debug_packets(
    outgoing: Option<&SocketAddr>,
    direction: PacketDirection,
    packetver: u32,
    buffer: &[u8],
    bytes_read: usize,
    name: &Option<String>,
) {
    let packet = parse(&buffer[..bytes_read], packetver);
    if packet.raw().len() < bytes_read {
        let mut offset = 0;
        while offset < bytes_read {
            let result = panic::catch_unwind(|| parse(&buffer[offset..bytes_read], packetver));
            if let Ok(packet) = result {
                offset += packet.raw().len();
                print_packet(name, outgoing, direction, &packet);
            } else {
                break;
            }
        }
    } else {
        print_packet(name, outgoing, direction, &packet);
    }
}

pub fn print_packet(name: &Option<String>, outgoing: Option<&SocketAddr>, direction: PacketDirection, packet: &Box<dyn Packet>) {
    if packet.id(GlobalConfigService::instance().packetver()) != "0x6003"
        && packet.id(GlobalConfigService::instance().packetver()) != "0x7f00"
        && packet.id(GlobalConfigService::instance().packetver()) != "0x0887"
        && packet.id(GlobalConfigService::instance().packetver()) != "0x7e00"
    {
        // PACKET_CZ_REQUEST_TIME2
        println!("\n----------------------------Start Packet----------------------------");
        if let Some(outgoing) = outgoing {
            info!(
                "{} {} {}",
                name.as_ref().cloned().unwrap_or("server".to_string()),
                if direction == PacketDirection::Backward { "<" } else { ">" },
                outgoing
            );
        } else {
            info!(
                "{} {} Client",
                name.as_ref().cloned().unwrap_or("".to_string()),
                if direction == PacketDirection::Backward { "<" } else { ">" }
            );
        }
        packet.display();
        packet.pretty_debug();
        info!("{:02X?}", packet.raw());
        println!("----------------------------End Packet----------------------------\n");
    }
}

/**
 Packets are buffered by session and flushed to the TCP Socket after a short delay.
 It allows to "send" packets from anywhere from the code, in the end packets will be send by batch and not one by one.
*/
pub struct PacketsBuffer {
    session_id: u32,
    max_capacity: usize,
    created_at: u128,
    data: Vec<u8>,
}

impl PacketsBuffer {
    pub fn new(session_id: u32, max_capacity: usize) -> Self {
        Self {
            session_id,
            max_capacity,
            created_at: get_tick(),
            data: Vec::with_capacity(2048),
        }
    }

    pub fn push(&mut self, new_data: &[u8]) {
        self.data.extend_from_slice(new_data);
    }

    pub fn should_flush(&self) -> bool {
        get_tick() - self.created_at > 20
    }

    pub fn can_contain(&self, len: usize) -> bool {
        self.data.len() + len <= self.max_capacity
    }

    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }

    pub fn session_id(&self) -> u32 {
        self.session_id
    }
}

#[cfg(test)]
mod tests {
    use packets::packets_parser::parse;

    use super::*;

    const VERSIONS: [u32; 4] = [20060101, 20120229, 20140101, 20160101];

    fn packet_id(raw: &[u8]) -> u16 {
        u16::from_le_bytes([raw[0], raw[1]])
    }

    #[test]
    fn notify_act_packet_id_by_packetver() {
        let expected = [0x008A, 0x02E1, 0x08C8, 0x08C8];
        for (packetver, expected_id) in VERSIONS.into_iter().zip(expected) {
            let raw = build_notify_act(packetver, NotifyAct {
                gid: 1,
                target_gid: 2,
                damage: 10,
                count: 1,
                ..Default::default()
            });
            assert_eq!(packet_id(&raw), expected_id, "packetver {packetver}");
        }
        assert_eq!(packet_id(&build_notify_act(20120307, NotifyAct::default())), 0x02E1);
    }

    #[test]
    fn notify_act_is_parsed_back_with_same_fields() {
        let act = NotifyAct {
            gid: 150001,
            target_gid: 2000001,
            attack_mt: 200,
            attacked_mt: 480,
            damage: 70000,
            count: 1,
            action: 0,
        };
        let packetver = 20120229;
        let raw = build_notify_act(packetver, act);
        let parsed = parse(&raw, packetver);
        let packet = parsed.as_any().downcast_ref::<PacketZcNotifyAct>().expect("PacketZcNotifyAct");
        assert_eq!(raw.len(), parsed.raw().len());
        assert_eq!(
            (packet.gid, packet.target_gid, packet.damage, packet.count),
            (150001, 2000001, 70000, 1)
        );
        assert_eq!((packet.attack_mt, packet.attacked_mt), (200, 480));
    }

    #[test]
    fn notify_act_short_damage_saturates_instead_of_wrapping() {
        let raw = build_notify_act(20060101, NotifyAct {
            damage: 70000,
            ..Default::default()
        });
        let parsed = parse(&raw, 20060101);
        let packet = parsed
            .as_any()
            .downcast_ref::<PacketZcNotifyActShortDamage>()
            .expect("PacketZcNotifyActShortDamage");
        assert_eq!(packet.damage, i16::MAX);
    }

    #[test]
    fn ack_req_name_all_packet_id_by_packetver() {
        let expected = [0x0195, 0x0195, 0x0195, 0x0A30];
        for (packetver, expected_id) in VERSIONS.into_iter().zip(expected) {
            let raw = build_ack_req_name_all(packetver, 1, [0 as char; 24]);
            assert_eq!(packet_id(&raw), expected_id, "packetver {packetver}");
        }
        assert_eq!(packet_id(&build_ack_req_name_all(20120307, 1, [0 as char; 24])), 0x0195);
    }

    #[test]
    fn notify_map_property_packet_id_by_packetver() {
        let expected = [0x0199, 0x0199, 0x099B, 0x099B];
        for (packetver, expected_id) in VERSIONS.into_iter().zip(expected) {
            let raw = build_notify_map_property(packetver, MAP_PROPERTY_NOTHING, 0);
            assert_eq!(packet_id(&raw), expected_id, "packetver {packetver}");
        }
        assert_eq!(packet_id(&build_notify_map_property(20120307, MAP_PROPERTY_NOTHING, 0)), 0x0199);
    }
}
