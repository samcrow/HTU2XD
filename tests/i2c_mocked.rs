extern crate embedded_hal;
extern crate embedded_hal_mock;
extern crate htu2xd;
extern crate nb;

use std::io;

use embedded_hal_mock::i2c::{Mock, Transaction};
use embedded_hal_mock::MockError;
use htu2xd::{Htu2xd, Reading, Resolution, SupplyVoltage};

/// Address of the sensor
const ADDRESS: u8 = 0x40;

/// Reads the default values from the user register, changes all the options, and writes them back
#[test]
fn user_register() -> Result<(), Box<dyn std::error::Error>> {
    // This is the default value, but with the three reserved bits (3, 4, and 5) set to 1.
    // The same values for those bits must be written back.
    let default_register_value = 0b0011_1010;
    // Written-back register value: reserved bits are the same, all other bits are flipped
    let new_register_value = 0b1011_1101;
    let expected = [
        Transaction::write_read(ADDRESS, vec![0b1110_0111], vec![default_register_value]),
        Transaction::write(ADDRESS, vec![0b1110_0110, new_register_value]),
    ];
    let mut mock = Mock::new(&expected);

    let mut htu = Htu2xd::new();
    let mut register = htu.read_user_register(&mut mock)?;

    // Check that the correct value was read
    assert!(matches!(
        register.resolution(),
        Resolution::Humidity12Temperature14
    ));
    assert!(matches!(register.supply_voltage(), SupplyVoltage::High));
    assert_eq!(register.otp_reload_enabled(), false);
    assert_eq!(register.heater_enabled(), false);

    // Change everything that can be changed
    register.set_resolution(Resolution::Humidity11Temperature11);
    register.set_heater_enabled(true);
    register.set_otp_reload_enabled(true);
    // Check that things were changed
    assert!(matches!(
        register.resolution(),
        Resolution::Humidity11Temperature11
    ));
    assert_eq!(register.otp_reload_enabled(), true);
    assert_eq!(register.heater_enabled(), true);
    // Write changes back
    htu.write_user_register(&mut mock, register)?;

    mock.done();
    Ok(())
}

#[test]
fn temperature_humidity_clock_stretch() {
    let expected = [
        // Read temperature
        Transaction::write_read(ADDRESS, vec![0xe3], vec![0x4e, 0x85, 0x6b]),
        // Read humidity
        Transaction::write_read(ADDRESS, vec![0xe5], vec![0x68, 0x3a, 0x7c]),
    ];
    let mut mock = Mock::new(&expected);

    let mut htu = Htu2xd::new();
    match htu.read_temperature_blocking(&mut mock).unwrap() {
        Reading::Ok(reading) => {
            assert_eq!(reading.as_raw(), 0x4e84);
            let degrees_c = reading.as_degrees_celsius();
            assert!(degrees_c >= 7.04);
            assert!(degrees_c < 7.05);
        }
        Reading::ErrorLow => panic!("Unexpected error low"),
        Reading::ErrorHigh => panic!("Unexpected error high"),
    }
    match htu.read_humidity_blocking(&mut mock).unwrap() {
        Reading::Ok(reading) => {
            assert_eq!(reading.as_raw(), 0x6838);
            let percent = reading.as_percent_relative();
            assert!(percent >= 44.8);
            assert!(percent < 44.9);
        }
        Reading::ErrorLow => panic!("Unexpected error low"),
        Reading::ErrorHigh => panic!("Unexpected error high"),
    }

    mock.done();
}

#[test]
fn temperature_humidity_nak() {
    /// A ConnectionRefused error here represents a NAK
    fn is_nak(error: &MockError) -> bool {
        matches!(error, MockError::Io(io::ErrorKind::ConnectionRefused))
    }

    let expected = [
        // Start temperature read
        Transaction::write(ADDRESS, vec![0xf3]),
        // Several read attempts that return NAK
        Transaction::read(ADDRESS, vec![0u8; 3])
            .with_error(MockError::Io(io::ErrorKind::ConnectionRefused)),
        Transaction::read(ADDRESS, vec![0u8; 3])
            .with_error(MockError::Io(io::ErrorKind::ConnectionRefused)),
        Transaction::read(ADDRESS, vec![0u8; 3])
            .with_error(MockError::Io(io::ErrorKind::ConnectionRefused)),
        Transaction::read(ADDRESS, vec![0u8; 3])
            .with_error(MockError::Io(io::ErrorKind::ConnectionRefused)),
        // Done measuring, return results
        Transaction::read(ADDRESS, vec![0x4e, 0x85, 0x6b]),
        // Start humidity read
        Transaction::write(ADDRESS, vec![0xf5]),
        // Several read attempts that return NAK
        Transaction::read(ADDRESS, vec![0u8; 3])
            .with_error(MockError::Io(io::ErrorKind::ConnectionRefused)),
        Transaction::read(ADDRESS, vec![0u8; 3])
            .with_error(MockError::Io(io::ErrorKind::ConnectionRefused)),
        // Done measuring, return results
        Transaction::read(ADDRESS, vec![0x68, 0x3a, 0x7c]),
    ];
    let mut mock = Mock::new(&expected);

    let mut htu = Htu2xd::new();

    let mut temperature_step2 = htu.read_temperature(&mut mock).unwrap();
    // 4 NAK errors while the sensor is measuring
    for _ in 0..4 {
        let error = temperature_step2
            .read_result(&mut mock, is_nak)
            .unwrap_err();
        assert!(matches!(error, nb::Error::WouldBlock));
    }
    // Now the sensor is done
    match temperature_step2.read_result(&mut mock, is_nak).unwrap() {
        Reading::Ok(reading) => {
            assert_eq!(reading.as_raw(), 0x4e84);
            let degrees_c = reading.as_degrees_celsius();
            assert!(degrees_c >= 7.04);
            assert!(degrees_c < 7.05);
        }
        Reading::ErrorLow => panic!("Unexpected error low"),
        Reading::ErrorHigh => panic!("Unexpected error high"),
    }
    let mut humidity_step2 = htu.read_humidity(&mut mock).unwrap();
    // 2 NAK errors while the sensor is measuring
    for _ in 0..2 {
        let error = humidity_step2.read_result(&mut mock, is_nak).unwrap_err();
        assert!(matches!(error, nb::Error::WouldBlock));
    }
    // Now the sensor is done
    match humidity_step2.read_result(&mut mock, is_nak).unwrap() {
        Reading::Ok(reading) => {
            assert_eq!(reading.as_raw(), 0x6838);
            let percent = reading.as_percent_relative();
            assert!(percent >= 44.8);
            assert!(percent < 44.9);
        }
        Reading::ErrorLow => panic!("Unexpected error low"),
        Reading::ErrorHigh => panic!("Unexpected error high"),
    }

    mock.done();
}
