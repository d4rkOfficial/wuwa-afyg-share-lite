// 工程文件 brotli 压缩（与原版 src/lib/project/compress.ts 对齐：原始 ≤5MB，压缩后 ≤0.5MB）

use anyhow::{bail, Result};
use brotli::{CompressorWriter, Decompressor};
use std::io::{Read, Write};

/// 原始工程文件上限 5MB
pub const MAX_RAW_BYTES: usize = 5 * 1024 * 1024;
/// 压缩后存储上限 0.5MB
pub const MAX_STORED_BYTES: usize = 512 * 1024;

/// 压缩前校验原始大小并抛错
pub fn assert_raw_size(text: &str) -> Result<()> {
    if text.len() > MAX_RAW_BYTES {
        bail!("工程文件超过 5MB 限制");
    }
    Ok(())
}

/// brotli 无损压缩，压缩后超上限抛错（quality=11 / lgwin=22 与 Node zlib 默认一致）
pub fn compress_project_text(text: &str) -> Result<Vec<u8>> {
    assert_raw_size(text)?;
    let mut out = Vec::new();
    {
        let mut w = CompressorWriter::new(&mut out, 4096, 11, 22);
        w.write_all(text.as_bytes())?;
        w.flush()?;
    }
    if out.len() > MAX_STORED_BYTES {
        bail!("工程压缩后仍过大");
    }
    Ok(out)
}

/// 解压为完整原始文本
pub fn decompress_project(blob: &[u8]) -> Result<String> {
    let mut r = Decompressor::new(blob, 4096);
    let mut out = String::new();
    r.read_to_string(&mut out)?;
    Ok(out)
}
