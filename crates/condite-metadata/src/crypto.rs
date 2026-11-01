use crate::{Config, Error, Result};
use std::io::{Read, Write};
use zeroize::Zeroizing;

// Fixed scrypt N=2^16, r=8, p=1 (~64 MiB), shared across native and WASM.
const WORK_FACTOR: u8 = 16;

pub(crate) fn encrypt(data: &[u8], passphrase: &str, config: &Config) -> Result<Vec<u8>> {
    if passphrase.trim().is_empty() || passphrase.len() > 4096 {
        return Err(Error::InvalidPassphrase);
    }
    let chunks = (data.len() as u64).div_ceil(65536).max(1);
    if (data.len() as u64).saturating_add(chunks * 16 + 1024) > config.max_output_bytes {
        return Err(Error::ResourceLimit);
    }
    let mut recipient = age::scrypt::Recipient::new(passphrase.to_owned().into());
    recipient.set_work_factor(WORK_FACTOR);
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|_| Error::InvalidPassphrase)?;
    let mut output = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut output)
        .map_err(|_| Error::ResourceLimit)?;
    writer.write_all(data).map_err(|_| Error::ResourceLimit)?;
    writer.finish().map_err(|_| Error::ResourceLimit)?;
    Ok(output)
}
/// Decrypts an age passphrase file; authentication failures never return partial plaintext.
pub fn decrypt(data: &[u8], passphrase: &str, config: &Config) -> Result<Vec<u8>> {
    config.check_input(data.len() as u64)?;
    if passphrase.trim().is_empty() || passphrase.len() > 4096 {
        return Err(Error::InvalidPassphrase);
    }
    let mut identity = age::scrypt::Identity::new(passphrase.to_owned().into());
    identity.set_max_work_factor(WORK_FACTOR);
    let decryptor = age::Decryptor::new(data).map_err(|_| Error::DecryptionFailed)?;
    let reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|_| Error::DecryptionFailed)?;
    let mut output = Zeroizing::new(Vec::new());
    reader
        .take(config.max_output_bytes.saturating_add(1))
        .read_to_end(&mut output)
        .map_err(|_| Error::DecryptionFailed)?;
    if output.len() as u64 > config.max_output_bytes {
        return Err(Error::ResourceLimit);
    }
    Ok(std::mem::take(&mut *output))
}
