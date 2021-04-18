#![no_std]

extern crate embedded_hal;
extern crate nb;

mod crc;
mod user_register;

pub use crate::user_register::{Resolution, SupplyVoltage, UserRegister};

use core::marker::PhantomData;
use core::slice;

use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

use crate::crc::Crc;

/// Address of the sensor
const ADDRESS: u8 = 0x40;

mod sealed {
    pub trait SealedFromRaw {
        fn from_raw(raw: u16) -> Self;
    }
}
use self::sealed::SealedFromRaw;

/// HTU2XD driver that does not own the I2C bus
///
/// The type parameter I is the I2C bus. This prevents one Htu2xd object from accidentally being
/// used with two different I2C peripherals.
///
/// # I2C type requirements
///
/// The `I` I2C bus type must return the same error type for read, write, and read/write operations.
///
/// # Examples
///
/// ## Configuration
///
/// ```no_run
/// use embedded_hal::blocking::delay::DelayMs;
/// use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
/// use htu2xd::{Htu2xd, Resolution};
///
/// fn init_htu2xd<I, E, D>(i2c: &mut I, delay: &mut D) -> Result<Htu2xd<I>, E>
/// where
///     I: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
///     D: DelayMs<u32>,
/// {
///     let mut htu = Htu2xd::new();
///
///     htu.soft_reset(i2c)?;
///     // Wait for the reset to finish
///     delay.delay_ms(15u32);
///
///     let mut register = htu.read_user_register(i2c)?;
///     register.set_resolution(Resolution::Humidity10Temperature13);
///     htu.write_user_register(i2c, register)?;
///
///     Ok(htu)
/// }
/// ```
///
/// ## Basic operation
///
/// ```no_run
/// use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
/// use htu2xd::{Htu2xd, Reading, Temperature};
/// fn use_sensor<I, E>(htu: &mut Htu2xd<I>, i2c: &mut I) -> Result<(), htu2xd::Error<E>>
/// where
///     I: Write<Error = E> + Read<Error = E> + WriteRead<Error = E>,
/// {
///     let temperature_reading = htu.read_temperature_blocking(i2c)?;
///     match temperature_reading {
///         Reading::Ok(reading) => {
///             println!("Temperature {} degrees C", reading.as_degrees_celsius())
///         }
///         Reading::ErrorLow => println!("Temperature off-scale low or sensor error"),
///         Reading::ErrorHigh => println!("Temperature off-scale high or sensor error"),
///     }
///     let humidity_reading = htu.read_humidity_blocking(i2c)?;
///     match humidity_reading {
///         Reading::Ok(reading) => println!("Humidity {}%", reading.as_percent_relative()),
///         Reading::ErrorLow => println!("Humidity off-scale low or sensor error"),
///         Reading::ErrorHigh => println!("Humidity off-scale high or sensor error"),
///     }
///     Ok(())
/// }
/// ```
///
/// ## Temperature and humidity reading without clock stretching
///
/// ```no_run
/// use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
/// use htu2xd::{Htu2xd, Reading, Temperature};
///
/// enum I2cError {
///     Nak,
///     OtherError,
/// }
///
/// impl I2cError {
///     fn is_nak(&self) -> bool {
///         matches!(self, I2cError::Nak)
///     }
/// }
///
/// fn use_sensor<I>(htu: &mut Htu2xd<I>, i2c: &mut I) -> Result<(), htu2xd::Error<I2cError>>
/// where
///     I: Write<Error = I2cError> + Read<Error = I2cError> + WriteRead<Error = I2cError>,
/// {
///     let mut temperature_step2 = htu.read_temperature(i2c)?;
///     // Do something else while the sensor is busy
///     // Later, read the result
///     let temperature_reading = nb::block!(temperature_step2.read_result(i2c, I2cError::is_nak))?;
///     match temperature_reading {
///         Reading::Ok(reading) => {
///             println!("Temperature {} degrees C", reading.as_degrees_celsius())
///         }
///         Reading::ErrorLow => println!("Temperature off-scale low or sensor error"),
///         Reading::ErrorHigh => println!("Temperature off-scale high or sensor error"),
///     }
///     let mut humidity_step2 = htu.read_humidity(i2c)?;
///     // Do something else while the sensor is busy
///     // Later, read the result
///     let humidity_reading = nb::block!(humidity_step2.read_result(i2c, I2cError::is_nak))?;
///     match humidity_reading {
///         Reading::Ok(reading) => println!("Humidity {}%", reading.as_percent_relative()),
///         Reading::ErrorLow => println!("Humidity off-scale low or sensor error"),
///         Reading::ErrorHigh => println!("Humidity off-scale high or sensor error"),
///     }
///     Ok(())
/// }
/// ```
pub struct Htu2xd<I>(PhantomData<I>);

impl<I, E> Htu2xd<I>
where
    I: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
{
    /// Creates a driver object, but does not perform any initialization
    pub fn new() -> Self {
        Htu2xd(PhantomData)
    }

    /// Resets the sensor and restores default settings, but does not restore the heater enable bit
    ///
    /// After this function returns the sensor may take up to 15 ms to reset.
    pub fn soft_reset(&mut self, i2c: &mut I) -> Result<(), E> {
        i2c.write(ADDRESS, &[Command::SoftReset as u8])
    }

    /// Reads the current humidity
    ///
    /// In this mode, the sensor stretches the I2C clock while it takes a measurement. This
    /// function blocks until the measurement has finished and been read.
    pub fn read_humidity_blocking(&mut self, i2c: &mut I) -> Result<Reading<Humidity>, Error<E>> {
        let mut buffer = [0u8; 3];
        i2c.write_read(ADDRESS, &[Command::HumidityHoldMaster as u8], &mut buffer)?;
        parse_and_check_reading(&buffer)
    }

    /// Reads the current temperature
    ///
    /// In this mode, the sensor stretches the I2C clock while it takes a measurement. This
    /// function blocks until the measurement has finished and been read.
    pub fn read_temperature_blocking(
        &mut self,
        i2c: &mut I,
    ) -> Result<Reading<Temperature>, Error<E>> {
        let mut buffer = [0u8; 3];
        i2c.write_read(
            ADDRESS,
            &[Command::TemperatureHoldMaster as u8],
            &mut buffer,
        )?;
        parse_and_check_reading(&buffer)
    }

    /// Reads the current humidity
    ///
    /// In this mode, the sensor does not stretch the I2C clock. After sending the command to
    /// the sensor, this function returns a proxy that can be polled to determine if the result
    /// is ready.
    pub fn read_humidity(&mut self, i2c: &mut I) -> Result<ResultReader<I, Humidity>, E> {
        // Send a command to start the read
        i2c.write(ADDRESS, &[Command::Humidity as u8])?;
        Ok(ResultReader {
            _driver: PhantomData,
            _reading: PhantomData,
        })
    }

    /// Reads the current temperature
    ///
    /// In this mode, the sensor does not stretch the I2C clock. After sending the command to
    /// the sensor, this function returns a proxy that can be polled to determine if the result
    /// is ready.
    pub fn read_temperature(&mut self, i2c: &mut I) -> Result<ResultReader<I, Temperature>, E> {
        // Send a command to start the read
        i2c.write(ADDRESS, &[Command::Temperature as u8])?;
        Ok(ResultReader {
            _driver: PhantomData,
            _reading: PhantomData,
        })
    }

    /// Reads the user register and returns its content
    pub fn read_user_register(&mut self, i2c: &mut I) -> Result<UserRegister, E> {
        let mut register_value = 0u8;
        i2c.write_read(
            ADDRESS,
            &[Command::ReadUser as u8],
            slice::from_mut(&mut register_value),
        )?;
        Ok(UserRegister(register_value))
    }

    /// Writes the user register
    ///
    /// You must use the `read_user_register` function to get a `UserRegister` object that
    /// can be modified and then passed to this function.
    pub fn write_user_register(&mut self, i2c: &mut I, register: UserRegister) -> Result<(), E> {
        i2c.write(ADDRESS, &[Command::WriteUser as u8, register.0])
    }
}

impl<I, E> Default for Htu2xd<I>
where
    I: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
{
    fn default() -> Self {
        Htu2xd::new()
    }
}

/// A proxy used to read the result of a non-blocking measurement
pub struct ResultReader<'h, I, M> {
    _driver: PhantomData<&'h mut Htu2xd<I>>,
    _reading: PhantomData<M>,
}

impl<'h, I, M> ResultReader<'h, I, M>
where
    I: Read,
    M: Measurement,
{
    /// Attempts to read a measurement result from the sensor
    ///
    /// is_nak must be a closure that returns true if the provided error is a NAK (negative
    /// acknowledge) error, or false otherwise.
    ///
    /// This function returns `Err(nb::Error::WouldBlock)` if the sensor does not acknowledge
    /// its address. This means that it is still performing the measurement. This function should
    /// be called again later to try again.
    ///
    /// On success, this function returns the sensor reading.
    ///
    /// After this function returns anything other than `Err(nb::Error::WouldBlock)`, this
    /// `ResultReader` must not be used again.
    pub fn read_result<F>(
        &mut self,
        i2c: &mut I,
        is_nak: F,
    ) -> nb::Result<Reading<M>, Error<I::Error>>
    where
        F: FnOnce(&I::Error) -> bool,
    {
        let mut buffer = [0u8; 3];
        match i2c.read(ADDRESS, &mut buffer[..]) {
            Ok(()) => parse_and_check_reading(&buffer).map_err(nb::Error::Other),
            Err(e) => {
                if is_nak(&e) {
                    // Measurement is still in progress, try again later
                    Err(nb::Error::WouldBlock)
                } else {
                    // Some other error, not a NAK
                    Err(nb::Error::Other(Error::I2c(e)))
                }
            }
        }
    }
}

/// Checks the CRC of a 3-byte temperature or humidity reading and parses it as a `Reading` object
fn parse_and_check_reading<M, E>(bytes: &[u8; 3]) -> Result<Reading<M>, Error<E>>
where
    M: Measurement,
{
    // Check CRC
    let mut crc = Crc::new();
    crc.add_all(&*bytes);
    if crc.value() != 0 {
        return Err(Error::Crc);
    }

    // Parse reading
    let reading16 = (u16::from(bytes[0]) << 8) | u16::from(bytes[1]);

    Ok(Reading::from_raw(reading16))
}

/// An I2C or CRC error
#[derive(Debug)]
pub enum Error<E> {
    /// The I2C driver returned an error
    I2c(E),
    /// A message was received from the sensor with an invalid CRC checksum
    Crc,
}

impl<E> From<E> for Error<E> {
    fn from(inner: E) -> Self {
        Error::I2c(inner)
    }
}

/// A temperature reading
#[derive(Debug, Clone)]
pub struct Temperature(u16);

impl Temperature {
    /// Returns the temperature reading exactly as read from the sensor, with the status bits
    /// cleared
    pub fn as_raw(&self) -> u16 {
        self.0
    }

    /// Converts the temperature reading into degrees Celsius
    ///
    /// This function uses single-precision floating-point operations.
    pub fn as_degrees_celsius(&self) -> f32 {
        -46.85_f32 + 175.72_f32 / 65536.0_f32 * f32::from(self.0)
    }
}

/// A humidity reading
#[derive(Debug, Clone)]
pub struct Humidity(u16);

impl Humidity {
    /// Returns the humidity reading exactly as read from the sensor, with the status bits cleared
    pub fn as_raw(&self) -> u16 {
        self.0
    }

    /// Converts the temperature reading into percent relative humidity (0.0 = 0%, 100.0 = 100%)
    /// and clamps it to the 0%-100% range
    ///
    /// This function uses single-precision floating-point operations.
    pub fn as_percent_relative(&self) -> f32 {
        -6.0_f32 + 125.0_f32 / 65536.0_f32 * f32::from(self.0)
    }
}

pub trait Measurement: SealedFromRaw {}
impl SealedFromRaw for Temperature {
    fn from_raw(raw: u16) -> Self {
        Temperature(raw)
    }
}
impl Measurement for Temperature {}
impl SealedFromRaw for Humidity {
    fn from_raw(raw: u16) -> Self {
        Humidity(raw)
    }
}
impl Measurement for Humidity {}

/// Information about a temperature or humidity reading
#[derive(Debug, Clone)]
pub enum Reading<R> {
    /// The reading was completed normally
    Ok(R),
    /// The reading was very low, or the sensor has an open circuit
    ErrorLow,
    /// The reading was very high, or the sensor has a short circuit
    ErrorHigh,
}

impl<R> Reading<R>
where
    R: Measurement,
{
    fn from_raw(raw: u16) -> Self {
        match raw {
            0x0000 => Reading::ErrorLow,
            0xffff => Reading::ErrorHigh,
            _ => {
                // Clear the status bits (lowest two) and return the reading
                Reading::Ok(R::from_raw(raw & 0xfffc))
            }
        }
    }
}

/// Commands to read and write things
enum Command {
    TemperatureHoldMaster = 0xe3,
    Temperature = 0xf3,
    HumidityHoldMaster = 0xe5,
    Humidity = 0xf5,
    WriteUser = 0xe6,
    ReadUser = 0xe7,
    SoftReset = 0xfe,
}
