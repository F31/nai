//! Internal migration primitive. No persistence, logging, retries or key deletion.
use crate::{
    foundation::{Operation, Result},
    llm::CredentialVault,
};
use nai_contracts::workbench::WorkbenchError as Error;
use serde_json::Value;

pub struct Identity<'a> {
    pub workspace: &'a str,
    pub model: &'a str,
    pub endpoint: &'a str,
}

/// A failed seal/readback may leave a target keyring entry. The transaction owner
/// must journal that boundary; this function never claims database atomicity.
pub fn rewrap(
    op: &Operation,
    source: &dyn CredentialVault,
    target: &dyn CredentialVault,
    from: Identity<'_>,
    to: Identity<'_>,
    sealed: &Value,
) -> Result<Value> {
    op.check()?;
    if from.endpoint != to.endpoint {
        return Err(Error::InvalidArgument);
    }
    let plain = source.open(from.workspace, from.model, from.endpoint, sealed)?;
    op.check()?;
    if plain.is_empty() || plain.len() > 8192 {
        return Err(Error::InvalidArgument);
    }
    let output = target.seal(to.workspace, to.model, to.endpoint, &plain)?;
    op.check()?;
    let verified = target.open(to.workspace, to.model, to.endpoint, &output)?;
    op.check()?;
    if plain.as_slice() != verified.as_slice() {
        return Err(Error::IntegrityFailed);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;
    use zeroize::Zeroizing;
    struct Vault {
        identity: &'static str,
        fail: bool,
        corrupt: bool,
        cancel: Option<Operation>,
        calls: Mutex<Vec<&'static str>>,
    }
    impl CredentialVault for Vault {
        fn seal(&self, ws: &str, id: &str, url: &str, secret: &[u8]) -> Result<Value> {
            self.calls.lock().unwrap().push("seal");
            assert_eq!(
                (ws, id, url),
                (self.identity, "model", "https://api.example/v1")
            );
            assert_eq!(secret, b"test-only-key");
            if self.fail {
                return Err(Error::PermissionDenied);
            }
            if let Some(op) = &self.cancel {
                op.cancel();
            }
            Ok(json!({"scope":ws,"opaque":"test-sealed"}))
        }
        fn open(
            &self,
            ws: &str,
            id: &str,
            url: &str,
            sealed: &Value,
        ) -> Result<Zeroizing<Vec<u8>>> {
            self.calls.lock().unwrap().push("open");
            assert_eq!(
                (ws, id, url),
                (self.identity, "model", "https://api.example/v1")
            );
            assert_eq!(sealed["scope"], ws);
            if self.fail {
                return Err(Error::PermissionDenied);
            }
            Ok(Zeroizing::new(if self.corrupt {
                b"wrong".to_vec()
            } else {
                b"test-only-key".to_vec()
            }))
        }
    }
    fn vault(identity: &'static str) -> Vault {
        Vault {
            identity,
            fail: false,
            corrupt: false,
            cancel: None,
            calls: Mutex::new(vec![]),
        }
    }
    fn identity(workspace: &str) -> Identity<'_> {
        Identity {
            workspace,
            model: "model",
            endpoint: "https://api.example/v1",
        }
    }
    fn run(op: &Operation, a: &Vault, b: &Vault) -> Result<Value> {
        rewrap(
            op,
            a,
            b,
            identity("original"),
            identity("shared"),
            &json!({"scope":"original"}),
        )
    }
    #[test]
    fn reencrypts_with_target_identity_and_verifies_before_delivery() {
        let a = vault("original");
        let b = vault("shared");
        let result = run(&Operation::new(4000).unwrap(), &a, &b).unwrap();
        assert_eq!(result, json!({"scope":"shared","opaque":"test-sealed"}));
        assert_eq!(*a.calls.lock().unwrap(), ["open"]);
        assert_eq!(*b.calls.lock().unwrap(), ["seal", "open"]);
    }
    #[test]
    fn source_failure_never_writes_target() {
        let mut a = vault("original");
        a.fail = true;
        let b = vault("shared");
        assert_eq!(
            run(&Operation::new(4000).unwrap(), &a, &b),
            Err(Error::PermissionDenied)
        );
        assert!(b.calls.lock().unwrap().is_empty());
    }
    #[test]
    fn corrupt_target_is_not_delivered() {
        let a = vault("original");
        let mut b = vault("shared");
        b.corrupt = true;
        assert_eq!(
            run(&Operation::new(4000).unwrap(), &a, &b),
            Err(Error::IntegrityFailed)
        );
    }
    #[test]
    fn cancellation_after_seal_does_not_readback_or_retry() {
        let op = Operation::new(4000).unwrap();
        let a = vault("original");
        let mut b = vault("shared");
        b.cancel = Some(op.clone());
        assert_eq!(run(&op, &a, &b), Err(Error::Cancelled));
        assert_eq!(*b.calls.lock().unwrap(), ["seal"]);
    }
    #[test]
    fn cancelled_operation_does_not_open_source() {
        let op = Operation::new(4000).unwrap();
        op.cancel();
        let a = vault("original");
        let b = vault("shared");
        assert_eq!(run(&op, &a, &b), Err(Error::Cancelled));
        assert!(a.calls.lock().unwrap().is_empty());
    }
    #[test]
    fn seal_failure_propagates_without_readback() {
        let a = vault("original");
        let mut b = vault("shared");
        b.fail = true;
        assert_eq!(
            run(&Operation::new(4000).unwrap(), &a, &b),
            Err(Error::PermissionDenied)
        );
        assert_eq!(*b.calls.lock().unwrap(), ["seal"]);
    }
}
