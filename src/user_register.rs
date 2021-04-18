//! Definitions for the user register

/// Resolution for temperature and humidity measurements
///
/// Lower resolutions take less time to measure.
#[derive(Debug, Clone)]
pub enum Resolution {
    /// 12-bit humidity, 14-bit temperature
    Humidity12Temperature14,
    /// 8-bit humidity, 12-bit temperature
    Humidity8Temperature12,
    /// 10-bit humidity, 13-bit temperature
    Humidity10Temperature13,
    /// 11-bit humidity, 11-bit temperature
    Humidity11Temperature11,
}

/// Measurement of the power supply voltage
///
/// Note: The sensor's minimum power supply voltage is 1.5 V.
#[derive(Debug, Clone)]
pub enum SupplyVoltage {
    /// Greater than 2.25 +/- 0.1 V
    High,
    /// Less than 2.25 +/- 0.1 V
    Low,
}

/// The user register, used for configuration
///
/// The only way to create a `UserRegister` object is to read it from a sensor. It can then be
/// modified and written back.
// The enclosed value is represented exactly as the sensor sends and receives it.
pub struct UserRegister(pub(crate) u8);

impl UserRegister {
    /// Returns the current measurement resolution
    pub fn resolution(&self) -> Resolution {
        let bit7 = ((self.0 >> 7) & 1) == 1;
        let bit0 = (self.0 & 1) == 1;
        match (bit7, bit0) {
            (false, false) => Resolution::Humidity12Temperature14,
            (false, true) => Resolution::Humidity8Temperature12,
            (true, false) => Resolution::Humidity10Temperature13,
            (true, true) => Resolution::Humidity11Temperature11,
        }
    }
    /// Returns the supply voltage when the last temperature or humidity measurement was taken
    pub fn supply_voltage(&self) -> SupplyVoltage {
        let bit6 = ((self.0 >> 6) & 1) == 1;
        if bit6 {
            SupplyVoltage::Low
        } else {
            SupplyVoltage::High
        }
    }
    /// Returns true if the on-chip heater is enabled
    pub fn heater_enabled(&self) -> bool {
        ((self.0 >> 2) & 1) == 1
    }
    /// Returns true if the one-time programmable memory reload is active
    ///
    /// With this active, the default settings will be restored after each temperature or
    /// humidity measurement.
    pub fn otp_reload_enabled(&self) -> bool {
        let bit1 = ((self.0 >> 1) & 1) == 1;
        // Invert (1 = disable)
        !bit1
    }

    ///  Sets the measurement resolution
    pub fn set_resolution(&mut self, resolution: Resolution) {
        let bit_7_mask = 1u8 << 7;
        let bit_0_mask = 1u8;
        // Clear bits 7 and 0
        self.0 &= !(bit_7_mask | bit_0_mask);
        match resolution {
            Resolution::Humidity12Temperature14 => { /* Leave both bits clear */ }
            Resolution::Humidity8Temperature12 => {
                // Set bit 0
                self.0 |= bit_0_mask;
            }
            Resolution::Humidity10Temperature13 => {
                // Set bit 7
                self.0 |= bit_7_mask;
            }
            Resolution::Humidity11Temperature11 => {
                // Set bits 7 and 0
                self.0 |= bit_7_mask | bit_0_mask;
            }
        }
    }
    /// Enables or disables the on-chip heater
    pub fn set_heater_enabled(&mut self, enabled: bool) {
        let bit_2_mask = 1u8 << 2;
        if enabled {
            self.0 |= bit_2_mask;
        } else {
            self.0 &= !bit_2_mask;
        }
    }
    /// Enables or disables the reloading of default settings from one-time programmable memory
    /// after each measurement
    pub fn set_otp_reload_enabled(&mut self, otp_reload: bool) {
        let bit_1_mask = 1u8 << 1;
        if otp_reload {
            // Clear bit (0 = enable)
            self.0 &= !bit_1_mask;
        } else {
            // Set bit (1 = disable)
            self.0 |= bit_1_mask;
        }
    }
}

mod debug_impl {
    use super::UserRegister;
    use core::fmt::{Debug, Formatter, Result};

    impl Debug for UserRegister {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            f.debug_struct("UserRegister")
                .field("resolution", &self.resolution())
                .field("supply_voltage", &self.supply_voltage())
                .field("heater_enabled", &self.heater_enabled())
                .field("otp_reload_enabled", &self.otp_reload_enabled())
                .finish()
        }
    }
}
