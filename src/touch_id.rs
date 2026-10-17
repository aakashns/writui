//! Unlocking with Touch ID, on Macs that have it.
//!
//! Turning it on makes a key in the Mac's Secure Enclave that can only be
//! used after a fingerprint, and encrypts the vault's key with it. Both are
//! kept in a small file outside the vault (see `config::touch_id_dir`):
//! the Secure Enclave key only as a reference that's useless on any other
//! Mac. Unlocking asks for a fingerprint to decrypt the vault's key, which
//! then opens the vault without the password (and without Argon2).
//!
//! No keychain: keychain items are tied to the app's code signature, and
//! writui isn't signed with a stable one, so every upgrade would ask for
//! the Mac's password before Touch ID could be used.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::config;
use crate::vault::{KEY_LEN, Key};

/// The first line of a Touch ID file, saying what it is.
const HEADER: &str = "writui Touch ID key, version 1";

/// Why unlocking with Touch ID didn't work.
pub enum Failure {
    /// The prompt was cancelled: type the password instead.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Cancelled,
    /// Something went wrong, said in a few words.
    Other(String),
}

/// Whether Touch ID can be used here: a Mac with fingerprints set up, and
/// writui running on it rather than over SSH (where the prompt would show
/// on a screen nobody's looking at).
pub fn available() -> bool {
    let remote = std::env::var_os("SSH_CONNECTION").is_some();
    !remote && platform::available()
}

/// Whether Touch ID is on for the vault at `vault`.
pub fn is_on(vault: &Path) -> bool {
    key_file(vault).is_ok_and(|file| file.exists())
}

/// Turn Touch ID on for the vault at `vault`, which opens with `key`.
/// Turning it on again starts over with a new Secure Enclave key.
pub fn turn_on(vault: &Path, key: &Key) -> Result<()> {
    let (reference, sealed) = platform::seal(key.as_ref())?;
    let file = key_file(vault)?;
    let dir = file.parent().context("no folder for the Touch ID key")?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir).with_context(|| format!("creating {}", dir.display()))?;
    let text = format!("{HEADER}\n{}\n{}\n", hex(&reference), hex(&sealed));
    write_private(&file, &text).with_context(|| format!("writing {}", file.display()))
}

/// Turn Touch ID off for the vault at `vault`.
pub fn turn_off(vault: &Path) -> Result<()> {
    let file = key_file(vault)?;
    match fs::remove_file(&file) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            Err(err).with_context(|| format!("removing {}", file.display()))
        }
        _ => Ok(()),
    }
}

/// Ask for a fingerprint and give back the vault's key. Blocks until the
/// prompt is answered.
pub fn unlock(vault: &Path) -> Result<Key, Failure> {
    let other = |err: anyhow::Error| Failure::Other(format!("{err:#}"));
    let file = key_file(vault).map_err(other)?;
    let text = fs::read_to_string(&file)
        .with_context(|| format!("reading {}", file.display()))
        .map_err(other)?;
    let mut lines = text.lines();
    let (Some(HEADER), Some(reference), Some(sealed)) = (lines.next(), lines.next(), lines.next())
    else {
        return Err(Failure::Other(format!("{} isn't a Touch ID key", file.display())));
    };
    let (Some(reference), Some(sealed)) = (unhex(reference), unhex(sealed)) else {
        return Err(Failure::Other(format!("{} is damaged", file.display())));
    };
    let opened = platform::open(&reference, &sealed)?;
    let mut key = Key::default();
    if opened.len() != KEY_LEN {
        return Err(Failure::Other("the Touch ID key is the wrong length".into()));
    }
    key.copy_from_slice(&opened);
    Ok(key)
}

/// The Touch ID file for the vault at `vault`, named after where the vault
/// is (so each vault has its own, and moving a vault turns it off).
fn key_file(vault: &Path) -> Result<PathBuf> {
    let vault = fs::canonicalize(vault).unwrap_or_else(|_| vault.to_path_buf());
    let digest = Sha256::digest(vault.as_os_str().as_encoded_bytes());
    Ok(config::touch_id_dir()?.join(hex(&digest[..16])))
}

fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    std::io::Write::write_all(&mut options.open(path)?, text.as_bytes())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

/// Everywhere but macOS: no Touch ID.
#[cfg(not(target_os = "macos"))]
mod platform {
    use super::Failure;
    use anyhow::{Result, bail};

    pub fn available() -> bool {
        false
    }

    pub fn seal(_key: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        bail!("Touch ID is only on Macs")
    }

    pub fn open(_reference: &[u8], _sealed: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>, Failure> {
        Err(Failure::Other("Touch ID is only on Macs".into()))
    }
}

/// The Secure Enclave, through Apple's Security and LocalAuthentication
/// frameworks.
#[cfg(target_os = "macos")]
mod platform {
    use std::ptr::{self, NonNull};

    use anyhow::{Result, anyhow};
    use objc2::rc::{Retained, autoreleasepool};
    use objc2_core_foundation::{
        CFBoolean, CFData, CFDictionary, CFError, CFNumber, CFRetained, CFString, CFType,
    };
    use objc2_foundation::NSString;
    use objc2_local_authentication::{LAContext, LAPolicy};
    use objc2_security::{
        SecAccessControl, SecAccessControlCreateFlags, SecKey, kSecAttrAccessControl,
        kSecAttrAccessibleWhenUnlockedThisDeviceOnly, kSecAttrIsPermanent, kSecAttrKeySizeInBits,
        kSecAttrKeyType, kSecAttrKeyTypeECSECPrimeRandom, kSecAttrTokenID,
        kSecAttrTokenIDSecureEnclave,
        kSecKeyAlgorithmECIESEncryptionCofactorVariableIVX963SHA256AESGCM as ECIES,
        kSecPrivateKeyAttrs, kSecUseAuthenticationContext,
    };
    use zeroize::Zeroizing;

    use super::Failure;

    /// The key attribute holding a Secure Enclave key's reference (Apple's
    /// `kSecAttrTokenOID`, which isn't in the public headers). Given back to
    /// the Secure Enclave, the reference brings the same key back.
    const TOKEN_OID: &str = "toid";

    /// What the Touch ID prompt says writui is trying to do.
    const REASON: &str = "unlock your writing";

    pub fn available() -> bool {
        autoreleasepool(|_| {
            let context = unsafe { LAContext::new() };
            unsafe {
                context.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics)
            }
            .is_ok()
        })
    }

    /// Make a Secure Enclave key that needs a fingerprint (one of those set
    /// up now: adding another one makes it stop working), and encrypt `key`
    /// with it. Gives back the Secure Enclave key's reference and the
    /// encrypted key.
    pub fn seal(key: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        autoreleasepool(|_| unsafe {
            let mut error = ptr::null_mut();
            let flags = SecAccessControlCreateFlags::PrivateKeyUsage
                | SecAccessControlCreateFlags::BiometryCurrentSet;
            let access = SecAccessControl::with_flags(
                None,
                cf(kSecAttrAccessibleWhenUnlockedThisDeviceOnly),
                flags,
                &mut error,
            )
            .ok_or_else(|| anyhow!("{}", describe(error)))?;
            let private = CFDictionary::<CFString, CFType>::from_slices(
                &[kSecAttrIsPermanent, kSecAttrAccessControl],
                &[cf(CFBoolean::new(false)), cf(&*access)],
            );
            let size = CFNumber::new_i32(256);
            let params = CFDictionary::<CFString, CFType>::from_slices(
                &[kSecAttrKeyType, kSecAttrKeySizeInBits, kSecAttrTokenID, kSecPrivateKeyAttrs],
                &[
                    cf(kSecAttrKeyTypeECSECPrimeRandom),
                    cf(&*size),
                    cf(kSecAttrTokenIDSecureEnclave),
                    cf(&*private),
                ],
            );
            let enclave_key = SecKey::new_random_key(params.as_opaque(), &mut error)
                .ok_or_else(|| anyhow!("making a Secure Enclave key: {}", describe(error)))?;
            let attributes =
                enclave_key.attributes().ok_or_else(|| anyhow!("no Secure Enclave key"))?;
            let attributes: &CFDictionary<CFString, CFType> = attributes.cast_unchecked();
            let reference = attributes
                .get(&CFString::from_static_str(TOKEN_OID))
                .and_then(|value| value.downcast::<CFData>().ok())
                .ok_or_else(|| anyhow!("the Secure Enclave key has no reference"))?;
            let public =
                enclave_key.public_key().ok_or_else(|| anyhow!("no Secure Enclave public key"))?;
            let sealed = public
                .encrypted_data(ECIES, &CFData::from_bytes(key), &mut error)
                .ok_or_else(|| anyhow!("encrypting the key: {}", describe(error)))?;
            Ok((reference.to_vec(), sealed.to_vec()))
        })
    }

    /// Bring back the Secure Enclave key from its reference, and decrypt
    /// `sealed` with it: this is when the fingerprint is asked for.
    pub fn open(reference: &[u8], sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, Failure> {
        autoreleasepool(|_| unsafe {
            let context = LAContext::new();
            context.setLocalizedReason(&NSString::from_str(REASON));
            // No "Use Password…" button: the vault's password is typed
            // into writui instead.
            context.setLocalizedFallbackTitle(Some(&NSString::from_str("")));
            let context_cf: &CFType = &*(Retained::as_ptr(&context) as *const CFType);
            let private = CFDictionary::<CFString, CFType>::from_slices(
                &[kSecAttrIsPermanent],
                &[cf(CFBoolean::new(false))],
            );
            let size = CFNumber::new_i32(256);
            let reference = CFData::from_bytes(reference);
            let token_oid = CFString::from_static_str(TOKEN_OID);
            let params = CFDictionary::<CFString, CFType>::from_slices(
                &[
                    kSecAttrKeyType,
                    kSecAttrKeySizeInBits,
                    kSecAttrTokenID,
                    &token_oid,
                    kSecUseAuthenticationContext,
                    kSecPrivateKeyAttrs,
                ],
                &[
                    cf(kSecAttrKeyTypeECSECPrimeRandom),
                    cf(&*size),
                    cf(kSecAttrTokenIDSecureEnclave),
                    cf(&*reference),
                    context_cf,
                    cf(&*private),
                ],
            );
            let mut error = ptr::null_mut();
            let Some(enclave_key) = SecKey::new_random_key(params.as_opaque(), &mut error) else {
                return Err(failure(error));
            };
            match enclave_key.decrypted_data(ECIES, &CFData::from_bytes(sealed), &mut error) {
                Some(key) => Ok(Zeroizing::new(key.to_vec())),
                None => Err(failure(error)),
            }
        })
    }

    fn cf<T: AsRef<CFType> + ?Sized>(value: &T) -> &CFType {
        value.as_ref()
    }

    fn take(error: *mut CFError) -> Option<CFRetained<CFError>> {
        NonNull::new(error).map(|error| unsafe { CFRetained::from_raw(error) })
    }

    fn describe(error: *mut CFError) -> String {
        match take(error) {
            Some(error) => error.description().map(|text| text.to_string()).unwrap_or_default(),
            None => "unknown error".into(),
        }
    }

    /// Cancelling the prompt (or the system cancelling it, e.g. when the
    /// Mac locks) is not an error: the password still works.
    fn failure(error: *mut CFError) -> Failure {
        let Some(error) = take(error) else { return Failure::Other("unknown error".into()) };
        let domain = error.domain().map(|domain| domain.to_string()).unwrap_or_default();
        let code = error.code();
        // LAError: user cancel, user fallback, system cancel, app cancel.
        let la_cancel =
            domain == "com.apple.LocalAuthentication" && [-2, -3, -4, -9].contains(&code);
        // errSecUserCanceled.
        let sec_cancel = domain == "NSOSStatusErrorDomain" && code == -128;
        if la_cancel || sec_cancel {
            return Failure::Cancelled;
        }
        Failure::Other(error.description().map(|text| text.to_string()).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_goes_both_ways() {
        let bytes = [0u8, 1, 0xab, 0xff];
        assert_eq!(hex(&bytes), "0001abff");
        assert_eq!(unhex("0001abff").unwrap(), bytes);
        assert!(unhex("abc").is_none() && unhex("zz").is_none());
    }
}
