#![allow(dead_code)]

use aes::Aes128;
use base64::Engine;
use block_padding::Pkcs7;
use cbc::cipher::KeyIvInit;
use cipher::KeyInit;
use cipher::block_padding::UnpadError;
use cipher::{BlockDecryptMut, BlockEncryptMut};
use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use rand::RngCore;
use rsa::{RsaPublicKey, pkcs8::DecodePublicKey, traits::PublicKeyParts};
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub enum CryptoMode {
    Weapi,
    Eapi,
    Linuxapi,
}

pub struct WeapiForm {
    pub params: String,
    pub enc_sec_key: String,
}

pub struct EapiForm {
    pub params: String,
}

pub struct LinuxapiForm {
    pub eparams: String,
}

const IV: &str = "0102030405060708";
const PRESET_KEY: &str = "0CoJUm6Qyw8W8jud";
const LINUXAPI_KEY: &str = "rFgB&h#%2?^eDg:Q";
const BASE62: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const EAPI_KEY: &str = "e82ckenh8dichen8";

const PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\n\
MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDgtQn2JZ34ZC28NWYpAUd98iZ3\n\
7BUrX/aKzmFbt7clFSs6sXqHauqKWqdtLkF2KexO40H1YTX8z2lSgBBOAxLsvakl\n\
V8k4cBFK9snQXE9/DDaFt6Rr7iVZMldczhC0JNgTz+SHXT6CBHuX3e9SdB1Ua44o\n\
ncaTWz7OBGLbCiK45wIDAQAB\n\
-----END PUBLIC KEY-----";

static RSA_PUBLIC_KEY: Lazy<Result<RsaPublicKey, rsa::pkcs8::spki::Error>> =
    Lazy::new(|| RsaPublicKey::from_public_key_pem(PUBLIC_KEY_PEM));

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;
type Aes128EcbEnc = ecb::Encryptor<Aes128>;
type Aes128EcbDec = ecb::Decryptor<Aes128>;

fn aes_128_cbc_encrypt_base64(pt: &[u8], key: &[u8], iv: &[u8]) -> Result<String, CryptoError> {
    let mut buf = pt.to_vec();
    let msg_len = buf.len();
    buf.resize(msg_len + 16, 0);
    let ct = Aes128CbcEnc::new(key.into(), iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, msg_len)
        .map_err(|_| CryptoError::EncryptPad)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(ct))
}

fn aes_128_ecb_encrypt_hex_upper(pt: &[u8], key: &[u8]) -> Result<String, CryptoError> {
    let mut buf = pt.to_vec();
    let msg_len = buf.len();
    buf.resize(msg_len + 16, 0);
    let ct = Aes128EcbEnc::new(key.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, msg_len)
        .map_err(|_| CryptoError::EncryptPad)?;
    Ok(hex::encode_upper(ct))
}

fn aes_128_ecb_decrypt_hex(ct_hex_upper: &str, key: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let ct = hex::decode(ct_hex_upper).map_err(CryptoError::BadHex)?;
    let mut buf = ct;
    let pt = Aes128EcbDec::new(key.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(CryptoError::BadPadding)?;
    Ok(pt.to_vec())
}

fn rsa_encrypt_none_hex(pt: &[u8]) -> Result<String, CryptoError> {
    let pk = RSA_PUBLIC_KEY
        .as_ref()
        .map_err(|e| CryptoError::BadPublicKey(e.to_string()))?;

    let mut padded = vec![0u8; 128usize.saturating_sub(pt.len())];
    padded.extend_from_slice(pt);

    let m = rsa::BigUint::from_bytes_be(&padded);
    let c = m.modpow(pk.e(), pk.n());
    let mut out = c.to_bytes_be();
    if out.len() < 128 {
        let mut left_pad = vec![0u8; 128 - out.len()];
        left_pad.append(&mut out);
        out = left_pad;
    }
    Ok(hex::encode(out))
}

fn random_base62_16() -> [u8; 16] {
    let mut rng = rand::thread_rng();
    let mut buf = [0u8; 16];
    let mut raw = [0u8; 16];
    rng.fill_bytes(&mut raw);
    let bytes = BASE62.as_bytes();
    for i in 0..16 {
        buf[i] = bytes[(raw[i] as usize) % 62];
    }
    buf
}

pub fn weapi(data: &Value) -> Result<WeapiForm, CryptoError> {
    let text = serde_json::to_string(data).map_err(CryptoError::BadJson)?;
    let sk = random_base62_16();

    let p1 = aes_128_cbc_encrypt_base64(text.as_bytes(), PRESET_KEY.as_bytes(), IV.as_bytes())?;
    let params = aes_128_cbc_encrypt_base64(p1.as_bytes(), &sk, IV.as_bytes())?;

    let mut reversed_sk = sk;
    reversed_sk.reverse();
    let enc_sec_key = rsa_encrypt_none_hex(&reversed_sk)?;

    Ok(WeapiForm {
        params,
        enc_sec_key,
    })
}

pub fn linuxapi(data: &Value) -> Result<LinuxapiForm, CryptoError> {
    let text = serde_json::to_string(data).map_err(CryptoError::BadJson)?;
    let eparams = aes_128_ecb_encrypt_hex_upper(text.as_bytes(), LINUXAPI_KEY.as_bytes())?;
    Ok(LinuxapiForm { eparams })
}

pub fn eapi(uri: &str, data: &Value) -> Result<EapiForm, CryptoError> {
    let text = serde_json::to_string(data).map_err(CryptoError::BadJson)?;
    let msg = format!("nobody{}use{}md5forencrypt", uri, text);

    let mut hasher = Md5::new();
    hasher.update(msg.as_bytes());
    let digest = hex::encode(hasher.finalize());

    let payload = format!("{uri}-36cd479b6b5-{text}-36cd479b6b5-{digest}");
    let params = aes_128_ecb_encrypt_hex_upper(payload.as_bytes(), EAPI_KEY.as_bytes())?;

    Ok(EapiForm { params })
}

pub fn eapi_res_decrypt_json(ct_hex_upper: &str) -> Result<Value, CryptoError> {
    let pt = aes_128_ecb_decrypt_hex(ct_hex_upper, EAPI_KEY.as_bytes())?;
    let s = String::from_utf8(pt).map_err(CryptoError::BadUtf8)?;
    serde_json::from_str(&s).map_err(CryptoError::BadJson)
}

#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    #[error("AES 加密 padding 错误")]
    EncryptPad,
    #[error("RSA 公钥解析失败: {0}")]
    BadPublicKey(String),
    #[error("hex 解码失败: {0}")]
    BadHex(hex::FromHexError),
    #[error("AES 解密 padding 错误: {0}")]
    BadPadding(UnpadError),
    #[error("UTF-8 解码失败: {0}")]
    BadUtf8(std::string::FromUtf8Error),
    #[error("JSON 解析失败: {0}")]
    BadJson(serde_json::Error),
}
