// 分享码生成（与原版 src/lib/utils/slug.ts 一致：8 位、去除易混淆字符）

use rand::Rng;

pub const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LENGTH: usize = 8;

pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    let mut code = String::with_capacity(CODE_LENGTH);
    for _ in 0..CODE_LENGTH {
        let idx = rng.gen_range(0..ALPHABET.len());
        code.push(ALPHABET[idx] as char);
    }
    code
}
