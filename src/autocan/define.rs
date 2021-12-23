#[derive(Clone, Copy, Debug)]
pub struct Header(pub [u8; 5]);

impl Header {
    #[inline]
    pub const fn new(
        network: u8,
        data_field: bool,
        proprity: u8,
        node_type: u8,
        node_index: u8,
        msg_type: u8,
    ) -> Self {
        Self([
            0xfe,
            (network << 6)
                | (if data_field { 1 << 5 } else { 0 })
                | (proprity << 2)
                | (node_type >> 4),
            ((node_type & 0xf) << 4) | (node_index),
            msg_type,
            0,
        ])
    }

    #[inline]
    #[allow(dead_code)]
    pub fn network(&self) -> u8 {
        self.0[1] >> 6
    }

    #[inline]
    pub fn data_field(&self) -> bool {
        self.0[1] & 0x20 != 0
    }

    #[inline]
    #[allow(dead_code)]
    pub fn proprity(&self) -> u8 {
        (self.0[1] & 0b11100) >> 2
    }

    #[inline]
    pub fn node_type(&self) -> u8 {
        ((self.0[1] & 0b11) << 4) | (self.0[2] >> 4)
    }

    #[inline]
    pub fn node_index(&self) -> u8 {
        self.0[2] & 0xf
    }

    #[inline]
    pub fn msg_type(&self) -> u8 {
        self.0[3]
    }
}
