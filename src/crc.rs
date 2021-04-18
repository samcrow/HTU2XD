//! CRC for communication integrity checks

/// 8-bit CRC, polynomial x^8 + x^5 + x^4 + 1, initialization 0
pub struct Crc {
    value: u8,
}

impl Crc {
    /// Creates a new CRC calculator initialized to zero
    pub fn new() -> Self {
        Crc { value: 0 }
    }

    /// Adds a byte to the CRC
    pub fn add(&mut self, byte: u8) {
        self.value ^= byte;
        for _ in 0..8 {
            if (self.value & 0x80) != 0 {
                self.value = (self.value << 1) ^ 0x31;
            } else {
                self.value <<= 1;
            }
        }
    }
    /// Adds bytes from a slice
    pub fn add_all(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.add(byte);
        }
    }
    /// Returns the current remainder value
    pub fn value(&self) -> u8 {
        self.value
    }
}

#[cfg(test)]
mod crc_test {
    use super::Crc;
    #[test]
    fn short() {
        let mut crc = Crc::new();
        crc.add(0xdc);
        assert_eq!(crc.value(), 0x79);
    }

    #[test]
    fn message_a() {
        let mut crc = Crc::new();
        crc.add_all(&[0x68, 0x3a]);
        assert_eq!(crc.value(), 0x7c);
    }

    #[test]
    fn message_b() {
        let mut crc = Crc::new();
        crc.add_all(&[0x4e, 0x85]);
        assert_eq!(crc.value(), 0x6b);
    }
}
