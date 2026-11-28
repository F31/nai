//! One bounded continuous audio task from sources in the same root context.
use nai_contracts::{v561_avatar::Render, workbench::WorkbenchError as Error};
use nai_runtime_core::foundation::Operation;
use sha2::{Digest, Sha256};
use std::{io::Write, path::Path, sync::Arc};

struct Identity {
    epoch: u64,
    portrait: String,
    operation: Operation,
}
#[derive(Clone)]
pub struct Context(Arc<Identity>);
pub struct Source {
    context: Context,
    id: u64,
    pcm: Vec<u8>,
    sha256: Vec<u8>,
}
pub struct Origin {
    pub id: u64,
    pub start: u64,
    pub samples: u64,
    pub sha256: Vec<u8>,
}
pub struct Batch {
    context: Context,
    file: tempfile::NamedTempFile,
    pcm: Vec<u8>,
    sha256: Vec<u8>,
    origins: Vec<Origin>,
    width: u32,
    height: u32,
}
impl Context {
    /// Receive canonical PCM incrementally, but expose a source only after the
    /// adapter verifies provider completion. This is not online GPU input.
    pub fn cosyvoice_source(
        &self,
        id: u64,
        config: &nai_llm_openai::cosyvoice::Config,
        text: &str,
    ) -> Result<(Source, nai_llm_openai::cosyvoice::Summary), Error> {
        self.0.operation.check()?;
        if id == 0 || config.max_samples == 0 || config.max_samples > 144000 {
            return Err(Error::InvalidArgument);
        }
        let operation = self.0.operation.clone();
        let (pcm, summary) = collect_verified(config.max_samples, &operation, |accept| {
            nai_llm_openai::cosyvoice::stream(config, text, operation.clone(), accept)
        })?;
        Ok((self.source(id, pcm)?, summary))
    }
    pub fn new(epoch: u64, portrait: &str, operation: Operation) -> Result<Self, Error> {
        operation.check()?;
        if epoch == 0 || portrait.len() != 64 || !portrait.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::InvalidArgument);
        }
        Ok(Self(Arc::new(Identity {
            epoch,
            portrait: portrait.to_owned(),
            operation,
        })))
    }
    /// Trusted root supplies completed provider PCM or an explicit manual snapshot.
    /// This seals byte identity; it does not verify a provider's EOS receipt.
    pub fn source(&self, id: u64, pcm: Vec<u8>) -> Result<Source, Error> {
        self.0.operation.check()?;
        if id == 0 || pcm.is_empty() || pcm.len() > 288000 || !pcm.len().is_multiple_of(2) {
            return Err(Error::InvalidArgument);
        }
        let sha256 = Sha256::digest(&pcm).to_vec();
        Ok(Source {
            context: self.clone(),
            id,
            pcm,
            sha256,
        })
    }
    pub fn invalidate(&self) {
        self.0.operation.cancel();
    }
}
fn collect_verified(
    max_samples: u64,
    operation: &Operation,
    receive: impl FnOnce(
        &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<nai_llm_openai::cosyvoice::Summary, Error>,
) -> Result<(Vec<u8>, nai_llm_openai::cosyvoice::Summary), Error> {
    operation.check()?;
    let mut pcm = Vec::new();
    let summary = receive(&mut |part| {
        operation.check()?;
        if part.is_empty() || !part.len().is_multiple_of(2) {
            return Err(Error::ProtocolError);
        }
        let length = pcm
            .len()
            .checked_add(part.len())
            .ok_or(Error::ResourceExhausted)?;
        if length as u64 > max_samples * 2 {
            return Err(Error::ResourceExhausted);
        }
        pcm.extend_from_slice(part);
        Ok(())
    })?;
    operation.check()?;
    if pcm.is_empty() || summary.samples != (pcm.len() / 2) as u64 {
        return Err(Error::ProtocolError);
    }
    Ok((pcm, summary))
}
pub fn frame_ends(lengths: &[u64]) -> Result<Vec<u64>, Error> {
    if lengths.is_empty() || lengths.len() > 2 || lengths.iter().any(|v| *v == 0 || *v > 144000) {
        return Err(Error::InvalidArgument);
    }
    let mut total = 0;
    let mut result = Vec::new();
    for size in lengths {
        total += size;
        if total > 144000 {
            return Err(Error::ResourceExhausted);
        }
        result.push((total * 25).div_ceil(24000));
    }
    Ok(result)
}
impl Batch {
    pub fn seal(
        context: &Context,
        sources: Vec<Source>,
        directory: &Path,
        width: u32,
        height: u32,
    ) -> Result<Self, Error> {
        context.0.operation.check()?;
        if sources.is_empty()
            || sources.len() > 2
            || !directory.is_absolute()
            || ![width, height]
                .iter()
                .all(|v| (256..=1024).contains(v) && v.is_multiple_of(32))
        {
            return Err(Error::InvalidArgument);
        }
        let mut total = 0u64;
        let mut previous = 0;
        for source in &sources {
            context.0.operation.check()?;
            if !Arc::ptr_eq(&context.0, &source.context.0)
                || source.id <= previous
                || Sha256::digest(&source.pcm).as_slice() != source.sha256
            {
                return Err(Error::PermissionDenied);
            }
            previous = source.id;
            total += (source.pcm.len() / 2) as u64;
        }
        if total > 144000 {
            return Err(Error::ResourceExhausted);
        }
        let mut pcm = Vec::new();
        let mut origins = Vec::new();
        for source in sources {
            context.0.operation.check()?;
            origins.push(Origin {
                id: source.id,
                start: (pcm.len() / 2) as u64,
                samples: (source.pcm.len() / 2) as u64,
                sha256: source.sha256,
            });
            pcm.extend_from_slice(&source.pcm);
        }
        let sha256 = Sha256::digest(&pcm).to_vec();
        let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| Error::IoFailed)?;
        file.write_all(&pcm)
            .and_then(|_| file.flush())
            .map_err(|_| Error::IoFailed)?;
        context.0.operation.check()?;
        Ok(Self {
            context: context.clone(),
            file,
            pcm,
            sha256,
            origins,
            width,
            height,
        })
    }
    pub fn render(&self) -> Result<Render, Error> {
        self.context.0.operation.check()?;
        Ok(Render {
            pcm_path: self
                .file
                .path()
                .to_str()
                .ok_or(Error::InvalidArgument)?
                .to_owned(),
            pcm_sha256: self.sha256.clone(),
            samples: (self.pcm.len() / 2) as u64,
            sample_rate: 24000,
        })
    }
    pub fn audio(&self) -> Result<Vec<u8>, Error> {
        self.context.0.operation.check()?;
        Ok(self.pcm.clone())
    }
    pub fn origins(&self) -> &[Origin] {
        &self.origins
    }
    pub fn portrait(&self) -> &str {
        &self.context.0.portrait
    }
    pub fn epoch(&self) -> u64 {
        self.context.0.epoch
    }
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> Context {
        Context::new(7, &"a".repeat(64), Operation::model_task(4000).unwrap()).unwrap()
    }
    fn summary(samples: u64) -> nai_llm_openai::cosyvoice::Summary {
        nai_llm_openai::cosyvoice::Summary {
            samples,
            first_pcm_ms: 1,
            complete_ms: 3,
            transport_eof_ms: 2,
        }
    }
    #[test]
    fn verified_stream_rejects_partial_failure_and_count_mismatch() {
        let op = Operation::model_task(4000).unwrap();
        assert!(matches!(
            collect_verified(4, &op, |accept| {
                accept(&[1, 0])?;
                Err(Error::OutcomeUnknown)
            }),
            Err(Error::OutcomeUnknown)
        ));
        assert!(matches!(
            collect_verified(4, &op, |accept| {
                accept(&[1, 0])?;
                Ok(summary(2))
            }),
            Err(Error::ProtocolError)
        ));
    }
    #[test]
    fn verified_stream_checks_cancel_after_completion_and_before_chunks() {
        for after in [false, true] {
            let op = Operation::model_task(4000).unwrap();
            assert!(matches!(
                collect_verified(4, &op, |accept| {
                    if !after {
                        op.cancel();
                    }
                    accept(&[1, 0])?;
                    op.cancel();
                    Ok(summary(1))
                }),
                Err(Error::Cancelled)
            ));
        }
    }
    #[test]
    fn verified_stream_is_bounded_and_preserves_exact_source_bytes() {
        let op = Operation::model_task(4000).unwrap();
        assert!(matches!(
            collect_verified(1, &op, |accept| {
                accept(&[1, 0, 2, 0])?;
                Ok(summary(2))
            }),
            Err(Error::ResourceExhausted)
        ));
        let (pcm, _) = collect_verified(2, &op, |accept| {
            accept(&[1, 0])?;
            accept(&[2, 0])?;
            Ok(summary(2))
        })
        .unwrap();
        let ctx = context();
        let dir = tempfile::tempdir().unwrap();
        let batch = Batch::seal(
            &ctx,
            vec![ctx.source(1, pcm.clone()).unwrap()],
            dir.path(),
            256,
            256,
        )
        .unwrap();
        assert_eq!(batch.audio().unwrap(), pcm);
        assert_eq!(batch.origins()[0].samples, 2);
        assert_eq!(batch.origins()[0].sha256, Sha256::digest(&pcm).to_vec());
    }
    #[test]
    fn exact_bytes_identity_and_single_tail_grid() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context();
        let sources = vec![
            ctx.source(1, vec![1; 1922]).unwrap(),
            ctx.source(2, vec![2; 1922]).unwrap(),
        ];
        let batch = Batch::seal(&ctx, sources, dir.path(), 256, 256).unwrap();
        let render = batch.render().unwrap();
        assert_eq!(
            std::fs::read(&render.pcm_path).unwrap(),
            [vec![1; 1922], vec![2; 1922]].concat()
        );
        assert_eq!(render.samples, 1922);
        assert_eq!(frame_ends(&[961, 961]).unwrap(), [2, 3]);
        assert_eq!(frame_ends(&[961, 1]).unwrap(), [2, 2]);
        let path = std::path::PathBuf::from(render.pcm_path);
        drop(batch);
        assert!(!path.exists());
    }
    #[test]
    fn same_labels_do_not_allow_cross_context_join() {
        let dir = tempfile::tempdir().unwrap();
        let operation = Operation::model_task(4000).unwrap();
        let a = Context::new(7, &"a".repeat(64), operation.clone()).unwrap();
        let b = Context::new(7, &"a".repeat(64), operation).unwrap();
        assert!(matches!(
            Batch::seal(
                &a,
                vec![
                    a.source(1, vec![0; 2]).unwrap(),
                    b.source(2, vec![0; 2]).unwrap()
                ],
                dir.path(),
                256,
                256
            ),
            Err(Error::PermissionDenied)
        ));
    }
    #[test]
    fn source_hash_change_and_duplicate_ids_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context();
        let mut source = ctx.source(1, vec![1; 2]).unwrap();
        source.pcm[0] = 2;
        assert!(Batch::seal(&ctx, vec![source], dir.path(), 256, 256).is_err());
        assert!(
            Batch::seal(
                &ctx,
                vec![
                    ctx.source(1, vec![0; 2]).unwrap(),
                    ctx.source(1, vec![0; 2]).unwrap()
                ],
                dir.path(),
                256,
                256
            )
            .is_err()
        );
    }
    #[test]
    fn cancelled_context_cannot_return_prepared_source() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context();
        let batch = Batch::seal(
            &ctx,
            vec![ctx.source(1, vec![0; 2]).unwrap()],
            dir.path(),
            256,
            256,
        )
        .unwrap();
        ctx.invalidate();
        assert_eq!(batch.render(), Err(Error::Cancelled));
        assert!(batch.audio().is_err());
    }
    #[test]
    fn bounded_sources_samples_and_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context();
        assert!(
            Batch::seal(
                &ctx,
                (1..=3)
                    .map(|id| ctx.source(id, vec![0; 2]).unwrap())
                    .collect(),
                dir.path(),
                256,
                256
            )
            .is_err()
        );
        assert!(
            Batch::seal(
                &ctx,
                vec![
                    ctx.source(1, vec![0; 288000]).unwrap(),
                    ctx.source(2, vec![0; 2]).unwrap()
                ],
                dir.path(),
                256,
                256
            )
            .is_err()
        );
        assert!(
            Batch::seal(
                &ctx,
                vec![ctx.source(1, vec![0; 288000]).unwrap()],
                dir.path(),
                2048,
                2048
            )
            .is_err()
        );
    }
    #[test]
    fn full_six_second_source_does_not_allocate_future_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context();
        let batch = Batch::seal(
            &ctx,
            vec![ctx.source(1, vec![0; 288000]).unwrap()],
            dir.path(),
            768,
            768,
        )
        .unwrap();
        assert_eq!(batch.render().unwrap().samples, 144000);
        assert_eq!(batch.audio().unwrap().len(), 288000);
    }
}
