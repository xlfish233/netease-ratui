use base64::Engine;
use md5::{Digest, Md5};
use rand::RngCore;

const ID_XOR_KEY_1: &str = "3go8&$8*3*3h0k(2)2";

pub fn generate_device_id() -> String {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut rng = rand::thread_rng();
    let mut out = String::with_capacity(52);
    for _ in 0..52 {
        let idx = (rng.next_u32() as usize) % HEX.len();
        out.push(HEX[idx] as char);
    }
    out
}

pub fn random_hex_string(bytes: usize) -> String {
    let mut rng = rand::thread_rng();
    let mut buf = vec![0u8; bytes];
    rng.fill_bytes(&mut buf);
    hex::encode(buf)
}

fn cloudmusic_dll_encode_id(device_id: &str) -> String {
    let mut xored = Vec::with_capacity(device_id.len());
    let key = ID_XOR_KEY_1.as_bytes();
    for (i, b) in device_id.as_bytes().iter().copied().enumerate() {
        xored.push(b ^ key[i % key.len()]);
    }

    let mut hasher = Md5::new();
    hasher.update(&xored);
    let digest = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(digest)
}

pub fn build_anonymous_username(device_id: &str) -> String {
    let encoded = cloudmusic_dll_encode_id(device_id);
    let s = format!("{device_id} {encoded}");
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

