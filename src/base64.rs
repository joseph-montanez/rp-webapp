// struct SimpleToken {
//     user_id: u32,
// }
//
// impl SimpleToken {
//     fn to_json(&self) -> String {
//         // Manual JSON serialization
//         format!("{{\"user_id\":{}}}", self.user_id)
//     }
// }

// pub fn serialize_to_json(user_id: u32, buffer: &mut [u8]) -> usize {
//     let json = format!("{{\"user_id\":{}}}", user_id);
//     let bytes = json.as_bytes();
//     let length = bytes.len().min(buffer.len());
//
//     buffer[..length].copy_from_slice(&bytes[..length]);
//     length
// }

pub fn base64_url_encode(input: &[u8], buffer: &mut [u8]) -> usize {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut index = 0;

    for chunk in input.chunks(3) {
        let len = chunk.len();
        let mut n = (chunk[0] as u32) << 16; // Ensure chunk[0] is cast to u32 before shifting
        if len > 1 {
            n |= (chunk[1] as u32) << 8; // Cast chunk[1] to u32 before shifting
        }
        if len > 2 {
            n |= chunk[2] as u32; // Cast chunk[2] to u32
        }

        buffer[index] = CHARSET[(n >> 18) as usize]; index += 1; // Cast to usize for indexing
        buffer[index] = CHARSET[((n >> 12) & 0x3F) as usize]; index += 1; // Cast to usize for indexing

        if len > 1 {
            buffer[index] = CHARSET[((n >> 6) & 0x3F) as usize]; index += 1; // Cast to usize for indexing
        }
        if len > 2 {
            buffer[index] = CHARSET[(n & 0x3F) as usize]; index += 1; // Cast to usize for indexing
        }
    }

    index
}

pub fn base64_url_decode(input: &[u8], buffer: &mut [u8]) -> usize {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut index = 0;
    let mut buffer_index = 0;

    while index < input.len() {
        let mut n = 0u32;
        let mut bits = 0u8;
        let mut char_count = 0u8;

        for _ in 0..4 {
            n <<= 6;
            if index < input.len() {
                let byte = input[index];
                index += 1;

                if byte != b'=' {
                    char_count += 1;
                    let val = CHARSET.iter().position(|&c| c == byte)
                        .unwrap_or(0) as u32; // Default to 0 for invalid characters
                    n |= val;
                }
            }
            bits += 6;
        }

        if char_count >= 2 { // At least two valid characters
            buffer[buffer_index] = (n >> 16) as u8;
            buffer_index += 1;
        }
        if char_count >= 3 { // At least three valid characters
            buffer[buffer_index] = (n >> 8) as u8;
            buffer_index += 1;
        }
        if char_count == 4 { // All four characters are valid
            buffer[buffer_index] = n as u8;
            buffer_index += 1;
        }
    }

    buffer_index
}

#[cfg(test)]
mod tests {
    use crate::base64::base64_url_decode;
    use super::*; // Import your http module functions

    #[test]
    fn test_encode_decode() {
        let value = b"Hello World!";
        let mut encoded_value = [0u8; 64];
        let mut decoded_value = [0u8; 64];

        let encoded_value_length = base64_url_encode(value, &mut encoded_value);
        let decoded_value_length = base64_url_decode(&encoded_value[..encoded_value_length], &mut decoded_value);


        println!("before value: {:?}", value);

        println!("before value: {}", core::str::from_utf8(value).unwrap_or("<invalid UTF-8>"));


        println!("after  value: {:?}", encoded_value);
        println!("after  value: {}", core::str::from_utf8(&encoded_value[..encoded_value_length]).unwrap_or("<invalid UTF-8>"));

        println!("after2 value: {:?}", decoded_value);
        println!("after2 value: {}", core::str::from_utf8(&decoded_value[..decoded_value_length]).unwrap_or("<invalid UTF-8>"));

    }
}