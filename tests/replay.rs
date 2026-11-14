//! Runs the built driver (`target/wasm32-unknown-unknown/release/driver_razer.wasm`) against
//! scripted exchanges based on the M0 capture (`captures/deathadder-v3-pro-receiver-2026-11-14.md`).
//! Build first: `cargo build --release --target wasm32-unknown-unknown`.

use waishe_driver_replay::types::cap::id;
use waishe_driver_replay::types::{
    EndpointInfo, HidInterface, ReadError, Unavailable, Value, WriteError, WriteOutcome,
};
use waishe_driver_replay::{DriverInstance, Replay, instantiate};

const WASM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasm32-unknown-unknown/release/driver_razer.wasm"
);
const OK: u8 = 0x02;
const BUSY: u8 = 0x01;
const NO_MOUSE: u8 = 0x04;

fn packet(status: u8, class: u8, id: u8, size: u8, args: &[u8]) -> [u8; 90] {
    let mut p = [0u8; 90];
    p[0] = status;
    p[1] = 0x1f;
    p[5] = size;
    p[6] = class;
    p[7] = id;
    p[8..8 + args.len()].copy_from_slice(args);
    p[88] = p[2..88].iter().fold(0, |a, b| a ^ b);
    p
}

fn hex(report_id_and_packet: &[u8]) -> String {
    report_id_and_packet
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The request the driver must send, and a reply.
fn send(class: u8, id: u8, size: u8, args: &[u8]) -> String {
    let mut r = vec![0u8];
    r.extend_from_slice(&packet(0, class, id, size, args));
    format!("send_feature 0: {}\n", hex(&r))
}

fn reply(status: u8, class: u8, id: u8, size: u8, args: &[u8]) -> String {
    let mut r = vec![0u8];
    r.extend_from_slice(&packet(status, class, id, size, args));
    format!("get_feature 0: {}\n", hex(&r))
}

fn ok(class: u8, id: u8, size: u8, req_args: &[u8], reply_args: &[u8]) -> String {
    send(class, id, size, req_args) + &reply(OK, class, id, size, reply_args)
}

const SERIAL: &[u8] = b"TEST00000000001";

fn probe_script() -> String {
    ok(0x00, 0x82, 0x16, &[], SERIAL) + &ok(0x00, 0x81, 0x02, &[], &[2, 2])
}

fn endpoint(pid: u16) -> EndpointInfo {
    EndpointInfo {
        vendor_id: 0x1532,
        product_id: pid,
        product: "Razer DeathAdder V3 Pro".into(),
        interfaces: vec![HidInterface {
            handle: 0,
            interface_number: 0,
            usage_page: 0x0001,
            usage: 0x0002,
        }],
    }
}

fn run(script: &str) -> DriverInstance<Replay> {
    let mut d = instantiate(WASM, Replay::parse(&(probe_script() + script)));
    let p = d.probe(&endpoint(0x00b7)).unwrap().unwrap();
    assert_eq!(p.model, "deathadder-v3-pro");
    assert_eq!(p.stable_id.as_deref(), Some("TEST00000000001"));
    assert_eq!(p.firmware.as_deref(), Some("2.2"));
    d
}

#[test]
fn unknown_product_is_not_mine() {
    let mut d = instantiate(WASM, Replay::parse(""));
    assert!(d.probe(&endpoint(0x00c3)).unwrap().is_err());
}

#[test]
fn reads_m0_values() {
    let mut d = run(&(ok(0x07, 0x80, 0x02, &[], &[0x00, 0xe3])
        + &ok(0x04, 0x85, 0x07, &[0x01], &[0x01, 0x03, 0x20, 0x03, 0x20])
        + &ok(0x00, 0x85, 0x01, &[], &[0x01])));
    assert_eq!(d.read(id::BATTERY_LEVEL).unwrap(), Ok(Value::Percent(89)));
    assert_eq!(
        d.read(id::MOUSE_DPI).unwrap(),
        Ok(Value::Dpi { x: 800, y: 800 })
    );
    assert_eq!(
        d.read(id::MOUSE_POLLING_RATE).unwrap(),
        Ok(Value::Integer(1000))
    );
    d.backend().assert_finished();
}

#[test]
fn mouse_off_is_disconnected() {
    // M0: with the mouse switched off the receiver answers every command with status 0x04.
    let off = || send(0x07, 0x80, 0x02, &[]) + &reply(NO_MOUSE, 0x07, 0x80, 0x02, &[]);
    let mut d = run(&(off() + &off()));
    assert_eq!(
        d.read(id::BATTERY_LEVEL).unwrap(),
        Err(ReadError::Unavailable(Unavailable::Disconnected))
    );
    assert_eq!(
        d.read(id::LINK_STATE).unwrap(),
        Ok(Value::Enum("disconnected".into()))
    );
    d.backend().assert_finished();
}

#[test]
fn busy_reply_is_read_again_without_resending() {
    let mut d = run(&(send(0x07, 0x80, 0x02, &[])
        + &reply(BUSY, 0x07, 0x80, 0x02, &[])
        + &reply(OK, 0x07, 0x80, 0x02, &[0x00, 0xe3])));
    assert_eq!(d.read(id::BATTERY_LEVEL).unwrap(), Ok(Value::Percent(89)));
    d.backend().assert_finished();
}

#[test]
fn corrupt_reply_resends() {
    let mut corrupt = vec![0u8];
    corrupt.extend_from_slice(&packet(OK, 0x07, 0x80, 0x02, &[0x00, 0xe3]));
    corrupt[89] ^= 0xff;
    let mut d = run(&(send(0x07, 0x80, 0x02, &[])
        + &format!("get_feature 0: {}\n", hex(&corrupt))
        + &ok(0x07, 0x80, 0x02, &[], &[0x00, 0xe3])));
    assert_eq!(d.read(id::BATTERY_LEVEL).unwrap(), Ok(Value::Percent(89)));
    d.backend().assert_finished();
}

#[test]
fn dpi_write_reads_back() {
    // M0 write: 04/05 with VARSTORE, 1600 x 1600, then read back.
    let mut d = run(&(ok(
        0x04,
        0x05,
        0x07,
        &[0x01, 0x06, 0x40, 0x06, 0x40, 0, 0],
        &[0x01, 0x06, 0x40, 0x06, 0x40],
    ) + &ok(0x04, 0x85, 0x07, &[0x01], &[0x01, 0x06, 0x40, 0x06, 0x40])));
    let v = Value::Dpi { x: 1600, y: 1600 };
    assert_eq!(
        d.write(id::MOUSE_DPI, &v).unwrap(),
        Ok(WriteOutcome::Applied(v))
    );
    d.backend().assert_finished();
}

#[test]
fn out_of_range_dpi_is_rejected_without_traffic() {
    let mut d = run("");
    assert!(matches!(
        d.write(id::MOUSE_DPI, &Value::Dpi { x: 50, y: 50 })
            .unwrap(),
        Err(WriteError::Rejected(_))
    ));
    assert!(matches!(
        d.write(id::MOUSE_POLLING_RATE, &Value::Integer(2000))
            .unwrap(),
        Err(WriteError::Rejected(_))
    ));
    d.backend().assert_finished();
}

#[test]
fn polling_write_reads_back() {
    let mut d =
        run(&(ok(0x00, 0x05, 0x01, &[0x02], &[0x02]) + &ok(0x00, 0x85, 0x01, &[], &[0x02])));
    assert_eq!(
        d.write(id::MOUSE_POLLING_RATE, &Value::Integer(500))
            .unwrap(),
        Ok(WriteOutcome::Applied(Value::Integer(500)))
    );
    d.backend().assert_finished();
}

#[test]
fn write_that_does_not_stick_is_an_error() {
    let mut d =
        run(&(ok(0x00, 0x05, 0x01, &[0x02], &[0x02]) + &ok(0x00, 0x85, 0x01, &[], &[0x01])));
    assert!(matches!(
        d.write(id::MOUSE_POLLING_RATE, &Value::Integer(500))
            .unwrap(),
        Err(WriteError::Protocol(_))
    ));
}
