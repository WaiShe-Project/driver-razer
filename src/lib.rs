// SPDX-License-Identifier: AGPL-3.0-or-later
//
// The exchange and retry logic is ported from OpenMouse mouse-protocol
// `src/drivers/razer/hid.ts` (`exchange`, `awaitReply`, `setPollingRate`;
// https://github.com/OpenMouse-Project/mouse-protocol, AGPL-3.0-or-later,
// commit 5a4f6433da3d70865d008874c7055016b336c197), copyright the OpenMouse contributors.

//! WaiShe driver for Razer mice.

pub mod codec;
pub mod devices;

use codec::{Command, ProtocolError, status};
use devices::Product;
use waishe_driver::hid::Handle;
use waishe_driver::{
    Access, CapabilityDescriptor, Constraint, Driver, EndpointInfo, Persistence, Precision,
    ProbeError, ProbeResult, ReadError, Transport, Unavailable, Value, ValueKind, WriteError,
    WriteOutcome, charging, id, link,
};

const RESPONSE_DELAY_MS: u32 = 100;
const RESPONSE_ATTEMPTS: usize = 6;
/// A polling-rate change briefly reconfigures the wireless link.
const POLLING_LINK_SETTLE_MS: u32 = 150;

/// Why an exchange produced no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Failure {
    Hid(Unavailable),
    /// The receiver answered for a mouse that does not respond (off or asleep).
    NoMouse,
    Unsupported,
    /// Busy or corrupt on every attempt.
    Busy,
    Status(u8),
}

impl From<Failure> for ReadError {
    fn from(f: Failure) -> Self {
        match f {
            Failure::Hid(u) => ReadError::Unavailable(u),
            Failure::NoMouse => ReadError::Unavailable(Unavailable::Disconnected),
            Failure::Unsupported => ReadError::Unsupported,
            Failure::Busy => ReadError::Unavailable(Unavailable::Busy),
            Failure::Status(s) => ReadError::Protocol(format!("status 0x{s:02x}")),
        }
    }
}

impl From<Failure> for WriteError {
    fn from(f: Failure) -> Self {
        match ReadError::from(f) {
            ReadError::Unavailable(u) => WriteError::Unavailable(u),
            ReadError::Unsupported => WriteError::Unsupported,
            ReadError::Protocol(p) => WriteError::Protocol(p),
        }
    }
}

pub struct Razer {
    control: Handle,
    product: &'static Product,
}

impl Razer {
    fn exchange(&self, cmd: &Command) -> Result<Vec<u8>, Failure> {
        let packet = codec::encode_request(cmd, self.product.transaction_id);
        let mut request = [0u8; codec::PACKET_LEN + 1];
        request[0] = codec::REPORT_ID;
        request[1..].copy_from_slice(&packet);
        // A busy status means the mouse will answer this same request later, so reads continue
        // without a re-send. A corrupt reply means the exchange itself was lost, so the send is
        // retried from scratch.
        for _ in 0..RESPONSE_ATTEMPTS {
            self.control
                .send_feature(&request)
                .map_err(|e| Failure::Hid(e.into()))?;
            if let Some(args) = self.await_reply(cmd)? {
                return Ok(args);
            }
        }
        Err(Failure::Busy)
    }

    fn await_reply(&self, cmd: &Command) -> Result<Option<Vec<u8>>, Failure> {
        let mut reply = [0u8; codec::PACKET_LEN + 1];
        for _ in 0..RESPONSE_ATTEMPTS {
            waishe_driver::sleep_ms(RESPONSE_DELAY_MS);
            reply.fill(0);
            reply[0] = codec::REPORT_ID;
            let n = self
                .control
                .get_feature(&mut reply)
                .map_err(|e| Failure::Hid(e.into()))?;
            let packet = &reply[1..n.clamp(1, reply.len())];
            match codec::decode_response(packet, cmd) {
                Ok(args) => return Ok(Some(args.to_vec())),
                Err(ProtocolError {
                    status: Some(status::BUSY),
                    ..
                }) => continue,
                // Corruption carries no status: transport noise rather than an answer.
                Err(ProtocolError { status: None, .. }) => return Ok(None),
                // Repeating a getter is safe; a setter must not be sent twice.
                Err(ProtocolError { stale: true, .. }) if cmd.is_getter() => return Ok(None),
                Err(ProtocolError {
                    status: Some(status::TIMEOUT),
                    ..
                }) => return Err(Failure::NoMouse),
                Err(ProtocolError {
                    status: Some(status::UNSUPPORTED),
                    ..
                }) => return Err(Failure::Unsupported),
                Err(ProtocolError {
                    status: Some(s), ..
                }) => return Err(Failure::Status(s)),
            }
        }
        Ok(None)
    }

    fn read_dpi(&self) -> Result<(u16, u16), Failure> {
        self.exchange(&codec::read_dpi_command(codec::STORAGE))
            .map(|a| codec::decode_dpi(&a))
    }

    fn read_polling(&self) -> Result<u32, Failure> {
        let args = self.exchange(&codec::READ_POLLING_RATE)?;
        codec::decode_legacy_polling_rate(&args).ok_or(Failure::Status(status::FAILURE))
    }

    fn polling_rates(&self) -> Vec<i64> {
        self.product
            .polling_rates
            .iter()
            .map(|&r| r as i64)
            .collect()
    }
}

impl Driver for Razer {
    fn probe(ep: &EndpointInfo) -> Result<(Self, ProbeResult), ProbeError> {
        let product = devices::find(ep.product_id).ok_or(ProbeError::NotMine)?;
        // The control interface is the one whose only collection is Generic Desktop Mouse.
        let control = ep
            .interfaces
            .iter()
            .find(|i| i.usage_page == 0x0001 && i.usage == 0x0002)
            .map(|i| Handle(i.handle))
            .ok_or(ProbeError::NotMine)?;
        let driver = Razer { control, product };
        // Read-only identification. A receiver whose mouse is off still probes successfully:
        // the endpoint exists, the mouse is just disconnected.
        let serial = driver
            .exchange(&codec::READ_SERIAL)
            .ok()
            .map(|a| codec::decode_serial(&a))
            .filter(|s| !s.is_empty());
        let firmware = driver
            .exchange(&codec::READ_FIRMWARE)
            .ok()
            .map(|a| codec::decode_firmware(&a));
        let transport = if product.wireless {
            Transport::Receiver
        } else {
            Transport::Wired
        };
        Ok((
            driver,
            ProbeResult {
                model: product.model.into(),
                stable_id: serial,
                transport,
                firmware,
                listen: vec![],
            },
        ))
    }

    fn describe(&self) -> Vec<CapabilityDescriptor> {
        let names = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect();
        vec![
            CapabilityDescriptor::new(id::BATTERY_LEVEL, Access::READ, ValueKind::Percent)
                .precision(Precision::Exact)
                .poll_every(300),
            CapabilityDescriptor::new(
                id::BATTERY_CHARGING,
                Access::READ,
                ValueKind::Enum {
                    variants: names(&[charging::DISCHARGING, charging::CHARGING]),
                },
            )
            .poll_every(300),
            CapabilityDescriptor::new(
                id::LINK_STATE,
                Access::READ,
                ValueKind::Enum {
                    variants: names(&[link::CONNECTED, link::DISCONNECTED]),
                },
            )
            .poll_every(300),
            CapabilityDescriptor::new(id::DEVICE_FIRMWARE, Access::READ, ValueKind::Text),
            CapabilityDescriptor::new(id::MOUSE_DPI, Access::READ_WRITE, ValueKind::Dpi)
                .constraint(Constraint::Dpi {
                    min: devices::DPI_MIN as u32,
                    max: self.product.max_dpi as u32,
                    step: devices::DPI_STEP as u32,
                    independent_xy: true,
                })
                .persistence(Persistence::Onboard)
                .poll_every(300),
            CapabilityDescriptor::new(
                id::MOUSE_POLLING_RATE,
                Access::READ_WRITE,
                ValueKind::Integer { unit: "Hz".into() },
            )
            .constraint(Constraint::Choices(self.polling_rates()))
            .persistence(Persistence::Onboard)
            .poll_every(300),
        ]
    }

    fn read(&mut self, capability: &str) -> Result<Value, ReadError> {
        match capability {
            id::BATTERY_LEVEL => Ok(Value::Percent(codec::decode_battery_percent(
                &self.exchange(&codec::READ_BATTERY)?,
            ))),
            id::BATTERY_CHARGING => {
                let charging = codec::decode_charging(&self.exchange(&codec::READ_CHARGING)?);
                Ok(Value::Enum(
                    (if charging {
                        charging::CHARGING
                    } else {
                        charging::DISCHARGING
                    })
                    .into(),
                ))
            }
            id::LINK_STATE => match self.exchange(&codec::READ_BATTERY) {
                Ok(_) => Ok(Value::Enum(link::CONNECTED.into())),
                Err(Failure::NoMouse) => Ok(Value::Enum(link::DISCONNECTED.into())),
                Err(e) => Err(e.into()),
            },
            id::DEVICE_FIRMWARE => Ok(Value::Text(codec::decode_firmware(
                &self.exchange(&codec::READ_FIRMWARE)?,
            ))),
            id::MOUSE_DPI => {
                let (x, y) = self.read_dpi()?;
                Ok(Value::Dpi {
                    x: x as u32,
                    y: y as u32,
                })
            }
            id::MOUSE_POLLING_RATE => Ok(Value::Integer(self.read_polling()? as i64)),
            _ => Err(ReadError::Unsupported),
        }
    }

    fn write(&mut self, capability: &str, value: &Value) -> Result<WriteOutcome, WriteError> {
        match (capability, value) {
            (id::MOUSE_DPI, &Value::Dpi { x, y }) => {
                let range = devices::DPI_MIN as u32..=self.product.max_dpi as u32;
                if !range.contains(&x) || !range.contains(&y) {
                    return Err(WriteError::Rejected(format!(
                        "DPI must be between {} and {}",
                        range.start(),
                        range.end()
                    )));
                }
                self.exchange(&codec::set_dpi_command(x as u16, y as u16, codec::STORAGE))?;
                let (rx, ry) = self.read_dpi()?;
                if (rx as u32, ry as u32) != (x, y) {
                    return Err(WriteError::Protocol(format!("the mouse kept {rx}x{ry}")));
                }
                Ok(WriteOutcome::Applied(Value::Dpi { x, y }))
            }
            (id::MOUSE_POLLING_RATE, &Value::Integer(hz)) => {
                let cmd = u32::try_from(hz)
                    .ok()
                    .filter(|hz| self.product.polling_rates.contains(hz))
                    .and_then(codec::set_legacy_polling_command)
                    .ok_or_else(|| {
                        WriteError::Rejected(format!("{hz} Hz is not supported on this connection"))
                    })?;
                self.exchange(&cmd)?;
                if self.product.wireless {
                    waishe_driver::sleep_ms(POLLING_LINK_SETTLE_MS);
                }
                let confirmed = self.read_polling()?;
                if confirmed as i64 != hz {
                    return Err(WriteError::Protocol(format!(
                        "the mouse kept {confirmed} Hz"
                    )));
                }
                Ok(WriteOutcome::Applied(Value::Integer(hz)))
            }
            (id::MOUSE_DPI | id::MOUSE_POLLING_RATE, _) => {
                Err(WriteError::Rejected("wrong value type".into()))
            }
            _ => Err(WriteError::Unsupported),
        }
    }
}

waishe_driver::export_driver!(Razer);
