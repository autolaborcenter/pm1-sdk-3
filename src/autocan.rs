mod define;

use define::Header;

/// 存放编码消息的缓冲区
///
/// -`len`: 已占用的空间
pub struct MessageBuffer<const LEN: usize> {
    buffer: [u8; LEN],
    cursor: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Message([u8; 14]);

pub struct MessageWriter<'a> {
    msg: &'a mut Message,
    cursor: usize,
}

pub struct MessageReader<'a> {
    msg: &'a Message,
    cursor: usize,
}

impl<const LEN: usize> Default for MessageBuffer<LEN> {
    #[inline]
    fn default() -> Self {
        Self {
            buffer: [0u8; LEN],
            cursor: 0,
        }
    }
}

impl<const LEN: usize> MessageBuffer<LEN> {
    #[inline]
    pub fn as_buf<'a>(&'a mut self) -> &'a mut [u8] {
        &mut self.buffer[self.cursor..]
    }

    #[inline]
    pub fn notify_received(&mut self, n: usize) {
        self.cursor += n;
    }

    #[inline]
    fn move_forward(&mut self, cursor: usize) {
        self.buffer.copy_within(cursor..self.cursor, 0);
        self.cursor -= cursor;
    }
}

impl<const LEN: usize> Iterator for MessageBuffer<LEN> {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        let mut cursor = 0usize;
        let size = 6usize;

        loop {
            // 找头
            while cursor < self.cursor && self.buffer[cursor] != 0xfe {
                cursor += 1;
            }
            if self.cursor < cursor + size {
                self.move_forward(cursor);
                return None;
            }

            // 确定包长
            let size = if unsafe { *(self.buffer[cursor..].as_ptr() as *const Header) }.data_field()
            {
                size + 8
            } else {
                size
            };
            if self.cursor < cursor + size {
                self.move_forward(cursor);
                return None;
            }

            // 校验
            if self.buffer[cursor..][size - 1] == crc_cauculate(&self.buffer[cursor..][1..size - 1])
            {
                let mut message = Message([0u8; 14]);
                message.0[..size].copy_from_slice(&self.buffer[cursor..][..size]);
                self.move_forward(cursor + size);
                return Some(message);
            } else {
                cursor += 1;
            }
        }
    }
}

impl Message {
    pub const fn new(
        network: u8,
        data_field: bool,
        proprity: u8,
        node_type: u8,
        node_index: u8,
        msg_type: u8,
    ) -> Self {
        let header = Header::new(
            network, data_field, proprity, node_type, node_index, msg_type,
        );
        let mut result = Self([0u8; 14]);
        let mut i = 0;
        while i < header.0.len() {
            result.0[i] = header.0[i];
            i += 1;
        }
        if !data_field {
            result.0[5] = crc_cauculate_const(&result.0, 1, 5);
        }
        result
    }

    #[inline]
    pub fn header<'a>(&'a self) -> &'a Header {
        unsafe { (self.0.as_ptr() as *const Header).as_ref() }.unwrap()
    }

    #[inline]
    pub fn as_slice<'a>(&'a self) -> &'a [u8] {
        if self.header().data_field() {
            &self.0
        } else {
            &self.0[..6]
        }
    }

    #[inline]
    pub fn write<'a>(&'a mut self) -> MessageWriter<'a> {
        MessageWriter {
            msg: self,
            cursor: 5,
        }
    }

    #[inline]
    pub fn read<'a>(&'a self) -> MessageReader<'a> {
        MessageReader {
            msg: self,
            cursor: 5,
        }
    }
}

impl MessageReader<'_> {
    pub unsafe fn read_unchecked<T: Copy>(&mut self) -> T {
        let slice = &self.msg.0[self.cursor..];
        let mut t = *(slice.as_ptr() as *const T);
        let len = std::mem::size_of_val(&t);
        std::slice::from_raw_parts_mut(&mut t as *mut T as *mut u8, len).reverse();
        self.cursor += len;
        t
    }
}

impl MessageWriter<'_> {
    pub unsafe fn write_unchecked<T: Sized>(&mut self, t: T) {
        let slice = &mut self.msg.0[self.cursor..];
        let len = std::mem::size_of::<T>();
        std::ptr::copy_nonoverlapping(&t, slice.as_mut_ptr() as *mut T, 1);
        slice[..len].reverse();
        self.cursor += len;
    }
}

impl Drop for MessageWriter<'_> {
    fn drop(&mut self) {
        if self.msg.header().data_field() {
            self.msg.0[13] = crc_cauculate(&self.msg.0[1..13]);
        } else {
            self.msg.0[5] = crc_cauculate(&self.msg.0[1..5]);
        }
    }
}

#[inline]
const fn crc_cauculate_const(buffer: &[u8], begin: usize, end: usize) -> u8 {
    let mut sum = 0u8;
    let mut i = begin;
    while i < end {
        sum = CRC8[(sum ^ buffer[i]) as usize];
        i += 1;
    }
    sum
}

#[inline]
const fn crc_cauculate(buffer: &[u8]) -> u8 {
    crc_cauculate_const(buffer, 0, buffer.len())
}

const CRC8: [u8; 256] = [
    0, 94, 188, 226, 97, 63, 221, 131, 194, 156, 126, 32, 163, 253, 31, 65, 157, 195, 33, 127, 252,
    162, 64, 30, 95, 1, 227, 189, 62, 96, 130, 220, 35, 125, 159, 193, 66, 28, 254, 160, 225, 191,
    93, 3, 128, 222, 60, 98, 190, 224, 2, 92, 223, 129, 99, 61, 124, 34, 192, 158, 29, 67, 161,
    255, 70, 24, 250, 164, 39, 121, 155, 197, 132, 218, 56, 102, 229, 187, 89, 7, 219, 133, 103,
    57, 186, 228, 6, 88, 25, 71, 165, 251, 120, 38, 196, 154, 101, 59, 217, 135, 4, 90, 184, 230,
    167, 249, 27, 69, 198, 152, 122, 36, 248, 166, 68, 26, 153, 199, 37, 123, 58, 100, 134, 216,
    91, 5, 231, 185, 140, 210, 48, 110, 237, 179, 81, 15, 78, 16, 242, 172, 47, 113, 147, 205, 17,
    79, 173, 243, 112, 46, 204, 146, 211, 141, 111, 49, 178, 236, 14, 80, 175, 241, 19, 77, 206,
    144, 114, 44, 109, 51, 209, 143, 12, 82, 176, 238, 50, 108, 142, 208, 83, 13, 239, 177, 240,
    174, 76, 18, 145, 207, 45, 115, 202, 148, 118, 40, 171, 245, 23, 73, 8, 86, 180, 234, 105, 55,
    213, 139, 87, 9, 235, 181, 54, 104, 138, 212, 149, 203, 41, 119, 244, 170, 72, 22, 233, 183,
    85, 11, 136, 214, 52, 106, 43, 117, 151, 201, 74, 20, 246, 168, 116, 42, 200, 150, 21, 75, 169,
    247, 182, 232, 10, 84, 215, 137, 107, 53,
];
