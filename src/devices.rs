// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Product facts ported from OpenMouse mouse-protocol `src/razer/devices.ts`
// (https://github.com/OpenMouse-Project/mouse-protocol, AGPL-3.0-or-later,
// commit 5a4f6433da3d70865d008874c7055016b336c197), copyright the OpenMouse contributors.
// Only DeathAdder V3 Pro entries that use the legacy polling command are included; 0x00C3 is left
// out until someone verifies which polling command it answers.

use crate::codec::TRANSACTION_ID_1F;

#[derive(Debug, Clone, Copy)]
pub struct Product {
    pub product_id: u16,
    /// `model` in `devices.toml`.
    pub model: &'static str,
    pub wireless: bool,
    pub polling_rates: &'static [u32],
    /// Sensor ceiling, per axis.
    pub max_dpi: u16,
    pub transaction_id: u8,
}

/// Rates the legacy polling command can encode.
pub const RATES_1K: &[u32] = &[125, 500, 1000];
const DPI_FOCUS_PRO: u16 = 30_000;

/// The sensor takes any whole DPI from here up to the ceiling, per axis.
pub const DPI_MIN: u16 = 100;
/// Synapse steps DPI by 50.
pub const DPI_STEP: u16 = 50;

pub const PRODUCTS: &[Product] = &[
    Product {
        product_id: 0x00b6,
        model: "deathadder-v3-pro",
        wireless: false,
        polling_rates: RATES_1K,
        max_dpi: DPI_FOCUS_PRO,
        transaction_id: TRANSACTION_ID_1F,
    },
    // OpenMouse: the extended polling command is accepted and reads back on this receiver, but the
    // measured rate stays at 1000 Hz, so the legacy command is used.
    Product {
        product_id: 0x00b7,
        model: "deathadder-v3-pro",
        wireless: true,
        polling_rates: RATES_1K,
        max_dpi: DPI_FOCUS_PRO,
        transaction_id: TRANSACTION_ID_1F,
    },
    Product {
        product_id: 0x00c2,
        model: "deathadder-v3-pro",
        wireless: false,
        polling_rates: RATES_1K,
        max_dpi: DPI_FOCUS_PRO,
        transaction_id: TRANSACTION_ID_1F,
    },
];

pub fn find(product_id: u16) -> Option<&'static Product> {
    PRODUCTS.iter().find(|p| p.product_id == product_id)
}
