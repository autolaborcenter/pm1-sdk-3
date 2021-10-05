use std::fmt::Display;

#[derive(Clone, Copy)]
pub struct Header(pub [u8; 5]);

impl Header {
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

    pub fn network(&self) -> u8 {
        self.0[1] >> 6
    }

    pub fn data_field(&self) -> bool {
        self.0[1] & 0x20 != 0
    }

    pub fn proprity(&self) -> u8 {
        (self.0[1] & 0b11100) >> 2
    }

    pub fn node_type(&self) -> u8 {
        ((self.0[1] & 0b11) << 4) | (self.0[2] >> 4)
    }

    pub fn node_index(&self) -> u8 {
        self.0[2] & 0xf
    }

    pub fn msg_type(&self) -> u8 {
        self.0[3]
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} | {}:{:02X}[{:1X}]:{:02X}",
            self.proprity(),
            self.network(),
            self.node_type(),
            self.node_index(),
            self.msg_type(),
        )
    }
}
