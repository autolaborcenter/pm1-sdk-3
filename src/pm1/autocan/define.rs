use std::fmt::Display;

use bit_field::BitField;

#[derive(Clone, Copy)]
pub struct Header([u8; 5]);

impl Default for Header {
    fn default() -> Self {
        Self([0xfe, 0, 0, 0, 0])
    }
}

impl Header {
    pub fn new(
        network: u8,
        data_field: bool,
        proprity: u8,
        node_type: u8,
        node_index: u8,
        msg_type: u8,
    ) -> Self {
        let mut result = Self::default();
        result.0[1].set_bits(6..=7, network);
        result.0[1].set_bit(5, data_field);
        result.0[1].set_bits(2..=4, proprity);
        result.0[1].set_bits(0..=1, node_type >> 4);
        result.0[2].set_bits(4..=7, node_type & 0b00_1111);
        result.0[2].set_bits(0..=3, node_index);
        result.0[3] = msg_type;
        result
    }

    pub fn network(&self) -> u8 {
        self.0[1].get_bits(6..=7)
    }

    pub fn data_field(&self) -> bool {
        self.0[1].get_bit(5)
    }

    pub fn proprity(&self) -> u8 {
        self.0[1].get_bits(2..=4)
    }

    pub fn node_type(&self) -> u8 {
        (self.0[1].get_bits(0..=1) << 4) + self.0[2].get_bits(4..=7)
    }

    pub fn node_index(&self) -> u8 {
        self.0[2].get_bits(0..=3)
    }

    pub fn msg_type(&self) -> u8 {
        self.0[3]
    }

    pub fn key(&self) -> u32 {
        unsafe { *(self.0[1..].as_ptr() as *const u32) }
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}[{}]: {}, {}",
            self.node_type(),
            self.node_index(),
            self.msg_type(),
            self.data_field()
        )
    }
}

pub struct HeaderChecker {
    mask: u32,
    template: u32,
}

impl HeaderChecker {
    pub fn new(
        network: Option<u8>,
        data_field: Option<bool>,
        proprity: Option<u8>,
        node_type: Option<u8>,
        node_index: Option<u8>,
        msg_type: Option<u8>,
    ) -> Self {
        let mask = !Header::new(
            network.map_or_else(|| 0u8, |_| 0b11),
            data_field.map_or_else(|| false, |_| true),
            proprity.map_or_else(|| 0u8, |_| 0b111),
            node_type.map_or_else(|| 0u8, |_| 0b_0011_1111),
            node_index.map_or_else(|| 0u8, |_| 0b1111),
            msg_type.map_or_else(|| 0u8, |_| 0xff),
        )
        .key();
        let template = Header::new(
            network.unwrap_or_else(|| 0),
            data_field.unwrap_or_else(|| false),
            proprity.unwrap_or_else(|| 0),
            node_type.unwrap_or_else(|| 0),
            node_index.unwrap_or_else(|| 0),
            msg_type.unwrap_or_else(|| 0),
        )
        .key();
        Self { mask, template }
    }

    pub fn check(&self, h: Header) -> bool {
        (h.key() & self.mask) == self.template
    }
}
