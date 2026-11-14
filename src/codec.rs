// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Ported to Rust from OpenMouse mouse-protocol `src/razer/codec.ts`
// (https://github.com/OpenMouse-Project/mouse-protocol, AGPL-3.0-or-later,
// commit 5a4f6433da3d70865d008874c7055016b336c197), copyright the OpenMouse contributors.
// Only the commands DeathAdder V3 Pro needs are ported. Changes: Rust types; errors as values.

//! Razer control protocol: 90-byte feature reports exchanged on report id 0.

pub const REPORT_ID: u8 = 0;
pub const PACKET_LEN: usize = 90;

/// Razer's transaction ids are named by value: they carry no meaning beyond "which firmware
/// generation answers to this", and a mismatch is silent — the mouse never replies.
pub const TRANSACTION_ID_1F: u8 = 0x1f;

const ARGS_OFFSET: usize = 8;
const CHECKSUM_INDEX: usize = 88;
const CHECKSUM_FIRST: usize = 2;
const CHECKSUM_LAST: usize = 88;
const BATTERY_SCALE: u32 = 255;
const MAX_ARGS: usize = PACKET_LEN - ARGS_OFFSET - 2;

pub mod status {
    pub const BUSY: u8 = 0x01;
    pub const OK: u8 = 0x02;
    pub const FAILURE: u8 = 0x03;
    pub const TIMEOUT: u8 = 0x04;
    pub const UNSUPPORTED: u8 = 0x05;
}

/// Firmware 1.12 of the Viper V3 Pro reports the same DPI from either store, and writes were
/// confirmed against this one, so reads and writes both use it.
pub const STORAGE: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub class: u8,
    pub id: u8,
    pub size: u8,
    args: [u8; 8],
    args_len: usize,
}

impl Command {
    const fn new(class: u8, id: u8, size: u8) -> Self {
        Command {
            class,
            id,
            size,
            args: [0; 8],
            args_len: 0,
        }
    }

    fn with_args(mut self, args: &[u8]) -> Self {
        self.args[..args.len()].copy_from_slice(args);
        self.args_len = args.len();
        self
    }

    pub fn args(&self) -> &[u8] {
        &self.args[..self.args_len]
    }

    /// Razer pairs each read with a write that clears the high bit of the command id. Repeating
    /// a question costs nothing; repeating an instruction is what must be avoided.
    pub fn is_getter(&self) -> bool {
        self.id & 0x80 != 0
    }
}

pub const READ_FIRMWARE: Command = Command::new(0x00, 0x81, 0x02);
pub const READ_SERIAL: Command = Command::new(0x00, 0x82, 0x16);
pub const READ_BATTERY: Command = Command::new(0x07, 0x80, 0x02);
pub const READ_CHARGING: Command = Command::new(0x07, 0x84, 0x02);
pub const READ_POLLING_RATE: Command = Command::new(0x00, 0x85, 0x01);
const READ_DPI: Command = Command::new(0x04, 0x85, 0x07);
const WRITE_DPI: Command = Command::new(0x04, 0x05, 0x07);
const WRITE_POLLING_RATE: Command = Command::new(0x00, 0x05, 0x01);

pub fn read_dpi_command(storage: u8) -> Command {
    READ_DPI.with_args(&[storage])
}

pub fn set_dpi_command(x: u16, y: u16, storage: u8) -> Command {
    WRITE_DPI.with_args(&[
        storage,
        (x >> 8) as u8,
        x as u8,
        (y >> 8) as u8,
        y as u8,
        0,
        0,
    ])
}

/// The legacy polling command encodes the rate as a divisor of 1000.
pub fn set_legacy_polling_command(hz: u32) -> Option<Command> {
    if hz == 0 || 1000 % hz != 0 || !(1..=0xff).contains(&(1000 / hz)) {
        return None;
    }
    Some(WRITE_POLLING_RATE.with_args(&[(1000 / hz) as u8]))
}

pub fn checksum(packet: &[u8; PACKET_LEN]) -> u8 {
    packet[CHECKSUM_FIRST..CHECKSUM_LAST]
        .iter()
        .fold(0, |a, b| a ^ b)
}

pub fn encode_request(cmd: &Command, transaction_id: u8) -> [u8; PACKET_LEN] {
    let mut p = [0u8; PACKET_LEN];
    p[1] = transaction_id;
    p[5] = cmd.size;
    p[6] = cmd.class;
    p[7] = cmd.id;
    p[ARGS_OFFSET..ARGS_OFFSET + cmd.args_len].copy_from_slice(cmd.args());
    p[CHECKSUM_INDEX] = checksum(&p);
    p
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError {
    /// `None` for transport corruption (bad length or checksum), which the mouse did not decide on.
    pub status: Option<u8>,
    /// The reply belonged to an earlier exchange, so re-reading may still find the right one.
    pub stale: bool,
}

/// Returns the reply arguments, or the reported status.
pub fn decode_response<'a>(packet: &'a [u8], cmd: &Command) -> Result<&'a [u8], ProtocolError> {
    let corrupt = ProtocolError {
        status: None,
        stale: false,
    };
    let p: &[u8; PACKET_LEN] = packet.try_into().map_err(|_| corrupt.clone())?;
    if p[CHECKSUM_INDEX] != checksum(p) {
        return Err(corrupt);
    }
    let st = p[0];
    if st != status::OK {
        return Err(ProtocolError {
            status: Some(st),
            stale: false,
        });
    }
    if p[6] != cmd.class || p[7] != cmd.id {
        return Err(ProtocolError {
            status: Some(st),
            stale: true,
        });
    }
    let len = (p[5] as usize).min(MAX_ARGS);
    Ok(&p[ARGS_OFFSET..ARGS_OFFSET + len])
}

/// JavaScript `Math.round(n / d)` for non-negative integers.
fn round_div(n: u32, d: u32) -> u32 {
    (2 * n + d) / (2 * d)
}

pub fn decode_firmware(args: &[u8]) -> String {
    format!("{}.{}", args[0], args[1])
}

pub fn decode_serial(args: &[u8]) -> String {
    args.iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn decode_battery_percent(args: &[u8]) -> u8 {
    round_div(args[1] as u32 * 100, BATTERY_SCALE) as u8
}

pub fn decode_charging(args: &[u8]) -> bool {
    args[1] == 1
}

pub fn decode_dpi(args: &[u8]) -> (u16, u16) {
    (
        u16::from_be_bytes([args[1], args[2]]),
        u16::from_be_bytes([args[3], args[4]]),
    )
}

pub fn decode_legacy_polling_rate(args: &[u8]) -> Option<u32> {
    (args[0] != 0).then(|| round_div(1000, args[0] as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_layout_matches_m0_capture() {
        // M0: battery request on the receiver, checksum over bytes 2..88.
        let p = encode_request(&READ_BATTERY, TRANSACTION_ID_1F);
        assert_eq!(&p[..8], &[0x00, 0x1f, 0x00, 0x00, 0x00, 0x02, 0x07, 0x80]);
        assert_eq!(p[88], 0x02 ^ 0x07 ^ 0x80);
    }

    #[test]
    fn decodes_m0_values() {
        assert_eq!(decode_battery_percent(&[0x00, 0xe3]), 89);
        assert_eq!(decode_dpi(&[0x01, 0x03, 0x20, 0x03, 0x20]), (800, 800));
        assert_eq!(decode_legacy_polling_rate(&[0x01]), Some(1000));
        assert_eq!(decode_legacy_polling_rate(&[0x02]), Some(500));
        assert_eq!(decode_legacy_polling_rate(&[0x08]), Some(125));
        assert_eq!(decode_firmware(&[2, 2]), "2.2");
    }

    #[test]
    fn polling_divisor_rejects_unencodable_rates() {
        assert_eq!(set_legacy_polling_command(500).unwrap().args(), &[0x02]);
        assert!(set_legacy_polling_command(2000).is_none());
        assert!(set_legacy_polling_command(300).is_none());
    }

    #[test]
    fn response_errors() {
        let mut p = [0u8; PACKET_LEN];
        p[0] = status::TIMEOUT;
        p[6] = 0x07;
        p[7] = 0x80;
        p[88] = checksum(&p);
        assert_eq!(
            decode_response(&p, &READ_BATTERY),
            Err(ProtocolError {
                status: Some(status::TIMEOUT),
                stale: false
            })
        );
        p[88] ^= 0xff;
        assert_eq!(
            decode_response(&p, &READ_BATTERY),
            Err(ProtocolError {
                status: None,
                stale: false
            })
        );
    }
}
