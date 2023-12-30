extern crate micro_ecc_sys;

use micro_ecc_sys::{uECC_Curve, uECC_make_key, uECC_sign, uECC_secp256r1, uECC_verify};
use crate::base64::base64_url_encode;
use sha2::{Sha256, Digest};

fn base64_encode(bytes: &[u8], buffer: &mut [u8]) {
    let hex_chars = b"0123456789abcdef"; // ASCII bytes for hexadecimal characters

    for (i, &byte) in bytes.iter().enumerate() {
        buffer[2 * i] = hex_chars[(byte >> 4) as usize];
        buffer[2 * i + 1] = hex_chars[(byte & 0x0F) as usize];
    }
}

fn base64_decode(input: &[u8], output: &mut [u8]) -> usize {
    let base64_chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output_index = 0;

    let mut current_bits = 0u32;
    let mut bit_count = 0u8;

    for &c in input.iter().filter(|&&c| c != b'=') {
        if let Some(val) = base64_chars.iter().position(|&b| b == c) {
            current_bits = (current_bits << 6) | val as u32;
            bit_count += 6;

            if bit_count >= 8 {
                bit_count -= 8;
                let byte = ((current_bits >> bit_count) & 0xFF) as u8;
                if output_index < output.len() {
                    output[output_index] = byte;
                    output_index += 1;
                }
            }
        }
    }

    output_index
}


fn hash_message(message: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(message);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

pub fn generate_keys() -> ([u8; 64], [u8; 32]) {
    let curve: uECC_Curve;
    unsafe {
        curve = uECC_secp256r1();
    }

    // Generate public and private keys
    let mut public_key = [0u8; 64];
    let mut private_key = [0u8; 32];
    unsafe {
        uECC_make_key(public_key.as_mut_ptr(), private_key.as_mut_ptr(), curve);
    }

    (public_key, private_key)
}

pub fn split_jwt(jwt: &[u8]) -> ( ([u8; 128], usize), ([u8; 128], usize), ([u8; 128], usize) ) {
    let mut header = [0u8; 128];
    let mut payload = [0u8; 128];
    let mut signature = [0u8; 128];

    let mut current_part = 0;
    let mut header_length = 0;
    let mut payload_length = 0;
    let mut signature_length = 0;

    for &byte in jwt {
        if byte == 0 {
            // Stop if a zero byte is encountered in the signature
            if current_part == 2 {
                break;
            }
        } else if byte == 46 { // ASCII value for '.'
            current_part += 1;
            continue;
        }

        match current_part {
            0 if header_length < 128 => {
                header[header_length] = byte;
                header_length += 1;
            },
            1 if payload_length < 128 => {
                payload[payload_length] = byte;
                payload_length += 1;
            },
            2 if signature_length < 128 => {
                signature[signature_length] = byte;
                signature_length += 1;
            },
            _ => {}
        }
    }

    ( (header, header_length), (payload, payload_length), (signature, signature_length) )
}

pub fn encode_token(private_key: &[u8; 32], jwt: &mut [u8; 256]) -> usize {
    let curve: uECC_Curve;
    unsafe {
        curve = uECC_secp256r1();
    }

    // Create header and payload
    let header = "{\"typ\":\"JWT\",\"alg\":\"ES256\"}";
    let payload = r#"{"user_id":"123","role":"admin"}"#;

    // Base64-url encode header and payload
    let mut encoded_header: [u8; 128]  = [0; 128];
    let mut encoded_payload: [u8; 128]  = [0; 128];
    let encoded_header_length = base64_url_encode(header.as_bytes(), &mut encoded_header);
    let encoded_payload_length = base64_url_encode(payload.as_bytes(), &mut encoded_payload);
    let mut message_length = encoded_header_length + encoded_payload_length;

    // println!("header: {}", core::str::from_utf8(&encoded_header[..encoded_header_length]).unwrap_or("<invalid UTF-8>"));
    // println!("payload: {}", core::str::from_utf8(&encoded_payload[..encoded_payload_length]).unwrap_or("<invalid UTF-8>"));

    // Concatenate header and payload
    let mut message: [u8; 256] = [0; 256];

    message[..encoded_header_length].copy_from_slice(&encoded_header[..encoded_header_length]);
    message[encoded_header_length..(encoded_header_length + 1)].copy_from_slice(b".");
    message[(encoded_header_length + 1)..(encoded_header_length + encoded_payload_length + 1)].copy_from_slice(&encoded_payload[..encoded_payload_length]);

    message_length = encoded_header_length + encoded_payload_length + 1;


    // Sign the message
    let mut signature = [0u8; 64];
    let mut message_hash= hash_message(&message[..message_length]);

    // println!("message: {}", core::str::from_utf8(&message[..message_length]).unwrap_or("<invalid UTF-8>"));
    // println!("message_hash: {:?}", message_hash);

    unsafe {
        uECC_sign(private_key.as_ptr(), message_hash.as_ptr(), 32, signature.as_mut_ptr(), curve);
    }

    // Base64-url encode the signature
    let mut encoded_signature: [u8; 128] = [0; 128];
    let encoded_signature_length = base64_url_encode(&signature, &mut encoded_signature);

    // println!("signature: {}", core::str::from_utf8(&encoded_signature[..encoded_signature_length]).unwrap_or("<invalid UTF-8>"));

    jwt[..message_length].copy_from_slice(&message[..message_length]);
    jwt[message_length..(message_length + 1)].copy_from_slice(b".");
    jwt[(message_length + 1)..(message_length + 1 + encoded_signature_length)].copy_from_slice(&encoded_signature[..encoded_signature_length]);

    let jwt_length = message_length + 1 + encoded_signature_length;

    return jwt_length;
}

pub fn verify_signature(public_key: &[u8; 64], header: &[u8], payload: &[u8], signature: &[u8]) -> bool {
    let curve: uECC_Curve;
    unsafe {
        curve = uECC_secp256r1();
    }

    // Construct the signed content
    let mut signed_content = [0u8; 256]; // Ensure this is large enough
    let signed_content_length = header.len() + payload.len() + 1; // +1 for the dot
    signed_content[..header.len()].copy_from_slice(header);
    signed_content[header.len()] = b'.';
    signed_content[header.len() + 1..signed_content_length].copy_from_slice(&payload[..payload.len()]);
    // println!("signed_content: {}", core::str::from_utf8(&signed_content[..signed_content_length]).unwrap_or("<invalid UTF-8>"));


    // Hash the signed content
    let message_hash = hash_message(&signed_content[..signed_content_length]);
    // println!("message_hash: {:?}", message_hash);

    // Verify the signature
    unsafe {
        return uECC_verify(public_key.as_ptr(), message_hash.as_ptr(), 32, signature.as_ptr(), curve) == 1
    }
}


#[cfg(test)]
mod tests {
    use crate::base64::base64_url_decode;
    use super::*; // Import your http module functions

    #[test]
    fn test_keys() {
        let (public_key, private_key) = generate_keys();

        let mut public_key_encoded = [0u8; 256];
        let mut private_key_encoded = [0u8; 128];

        let public_key_encoded_length = base64_url_encode(&public_key, &mut public_key_encoded);
        let private_key_encoded_length = base64_url_encode(&private_key, &mut private_key_encoded);

        println!("public_key_encoded: {}", core::str::from_utf8(&public_key_encoded).unwrap_or("<invalid UTF-8>"));
        println!("private_key_encoded: {}", core::str::from_utf8(&private_key_encoded).unwrap_or("<invalid UTF-8>"));

        let mut public_key_decoded = [0u8; 128];
        let mut private_key_decoded = [0u8; 64];

        let public_key2_length = base64_url_decode(&public_key_encoded[..public_key_encoded_length], &mut public_key_decoded);

        let private_key2_length = base64_url_decode(&private_key_encoded[..private_key_encoded_length], &mut private_key_decoded);

        println!("before public_key({}): {:?}", 64, public_key);
        println!("after  public_key({}): {:?}", public_key2_length, &public_key_decoded[..public_key2_length]);

        println!("before private_key({}): {:?}", 32, private_key);
        println!("after  private_key({}): {:?}", private_key2_length, &private_key_decoded[..private_key2_length]);

    }

    #[test]
    fn test_encoding() {
        let (public_key, private_key) = generate_keys();

        println!("public_key: {:?}", public_key);
        println!("private_key: {:?}", private_key);

        let public_key_encoded = b"cYXtSQ-MQw3kKeWeB2oaZIsL7-vJ784YZt1xvMQly4N3d1lTyE7spPnWK3f6-rVAH7JaJMFnAmsJJ-FgHl2F0A==";
        let private_key_encoded = b"Djy0GC2Z7uCKXMRWmyScrBOu5-NPjm0i84NY1bAgp5w=";

        let mut public_key = [0u8; 64];
        let mut private_key = [0u8; 32];

        let public_key_length = base64_url_decode(public_key_encoded, &mut public_key);

        let private_key_length = base64_url_decode(private_key_encoded, &mut private_key);

        println!("public_key: {:?}", public_key);
        println!("private_key: {:?}", private_key);

        let mut jwt = [0u8; 256];
        let jwt_length = encode_token(&private_key, &mut jwt);
        let expected_jwt = b"eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiJ9.eyJ1c2VyX2lkIjoiMTIzIiwicm9sZSI6ImFkbWluIn0=.dK-v_eODrpbFDLsspeaBi8vQa8PNz4lDqGAPHtXJBHwKqLKH5i4moewhYyceHlrmRDzjX9rWe2tDbp6LDYBqXg==";

        println!("expected jwt({}): {}", 170, core::str::from_utf8(&expected_jwt[..170]).unwrap_or("<invalid UTF-8>"));
        println!("acquired jwt({}): {}", jwt_length, core::str::from_utf8(&jwt[..jwt_length]).unwrap_or("<invalid UTF-8>"));

        let (header_part, payload_part, signature_part) = split_jwt(&jwt);

        let header = &header_part.0[..header_part.1];
        let payload = &payload_part.0[..payload_part.1];
        let mut signature = [0u8; 64];
        let signature_length = base64_url_decode(&signature_part.0[..signature_part.1], &mut signature);

        println!("JWT Header({}): {}", header.len(), core::str::from_utf8(header).unwrap_or("<invalid UTF-8>"));
        println!("JWT Payload({}): {}", payload.len(), core::str::from_utf8(payload).unwrap_or("<invalid UTF-8>"));
        println!("JWT Signature({}): {}", signature_part.1, core::str::from_utf8(&signature_part.0[..signature_part.1]).unwrap_or("<invalid UTF-8>"));

        let valid = verify_signature(&public_key, header, payload, &signature[..signature_length]);

        assert_eq!(valid, true);
    }
}
