//! V5.6.1 system credential references.
//!
//! Config keeps only `api_key_ref = "keyring:<service>/<account>"`; the secret
//! itself lives in the OS credential store. Non-Windows builds compile the
//! parser but return CapabilityUnavailable for storage/read.

use nai_contracts::workbench::WorkbenchError as Error;
use zeroize::Zeroizing;

pub fn parse(reference: &str) -> Result<(String, String), Error> {
    let rest = reference
        .strip_prefix("keyring:")
        .ok_or(Error::InvalidArgument)?;
    let (service, account) = rest.split_once('/').ok_or(Error::InvalidArgument)?;
    if service.is_empty()
        || account.is_empty()
        || service.len() > 128
        || account.len() > 256
        || !service
            .bytes()
            .chain(account.bytes())
            .all(|b| b.is_ascii_graphic() && b != b'/')
    {
        return Err(Error::InvalidArgument);
    }
    Ok((service.to_string(), account.to_string()))
}

pub fn load(reference: &str) -> Result<Zeroizing<Vec<u8>>, Error> {
    let (service, account) = parse(reference)?;
    #[cfg(windows)]
    {
        keyring::Entry::new(&service, &account)
            .map_err(|_| Error::CapabilityUnavailable)?
            .get_secret()
            .map(Zeroizing::new)
            .map_err(|_| Error::CapabilityUnavailable)
    }
    #[cfg(not(windows))]
    {
        let _ = (service, account);
        Err(Error::CapabilityUnavailable)
    }
}

pub fn store(reference: &str, secret: &[u8]) -> Result<(), Error> {
    if secret.is_empty() || secret.len() > 4096 || secret.iter().any(|b| *b < 33 || *b > 126) {
        return Err(Error::InvalidArgument);
    }
    let (service, account) = parse(reference)?;
    #[cfg(windows)]
    {
        use zeroize::Zeroize;
        let mut copy = secret.to_vec();
        let result = keyring::Entry::new(&service, &account)
            .map_err(|_| Error::CapabilityUnavailable)?
            .set_secret(&copy)
            .map_err(|_| Error::CapabilityUnavailable);
        copy.zeroize();
        result
    }
    #[cfg(not(windows))]
    {
        let _ = (service, account, secret);
        Err(Error::CapabilityUnavailable)
    }
}

pub fn delete(reference: &str) -> Result<(), Error> {
    let (service, account) = parse(reference)?;
    #[cfg(windows)]
    {
        keyring::Entry::new(&service, &account)
            .map_err(|_| Error::CapabilityUnavailable)?
            .delete_credential()
            .map_err(|_| Error::CapabilityUnavailable)
    }
    #[cfg(not(windows))]
    {
        let _ = (service, account);
        Err(Error::CapabilityUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_explicit_keyring_refs() {
        assert_eq!(
            parse("keyring:nai/llm").unwrap(),
            ("nai".into(), "llm".into())
        );
        assert_eq!(
            parse("keyring:nai.tts/default-voice").unwrap(),
            ("nai.tts".into(), "default-voice".into())
        );
        for bad in [
            "",
            "plain-key",
            "keyring:",
            "keyring:/llm",
            "keyring:nai/",
            "keyring:nai/llm/extra",
            "keyring:nai llm/acc",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn non_windows_storage_is_explicitly_unavailable() {
        #[cfg(not(windows))]
        {
            assert_eq!(
                load("keyring:nai/llm").unwrap_err(),
                Error::CapabilityUnavailable
            );
            assert_eq!(
                store("keyring:nai/llm", b"synthetic").unwrap_err(),
                Error::CapabilityUnavailable
            );
            assert_eq!(
                delete("keyring:nai/llm").unwrap_err(),
                Error::CapabilityUnavailable
            );
        }
    }
}
