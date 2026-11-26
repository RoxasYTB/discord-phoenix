use openmls::prelude::tls_codec::{Deserialize as _, Serialize as _};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;

use super::mls::{CommitWelcome, MlsSession};
use crate::{Error, Result};

pub const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMP256_AES128GCM_SHA256_P256;

pub fn key_package_lifetime() -> openmls::key_packages::Lifetime {
    openmls::key_packages::Lifetime::init(0, u64::MAX)
}

fn mls<E: std::fmt::Display>(ctx: &'static str) -> impl Fn(E) -> Error {
    move |e| Error::Mls(format!("{ctx}: {e}"))
}

pub fn credential_user_id(c: &Credential) -> Option<u64> {
    let b = BasicCredential::try_from(c.clone()).ok()?;
    let id: [u8; 8] = b.identity().try_into().ok()?;
    Some(u64::from_be_bytes(id))
}

fn read_vl(bytes: &[u8]) -> Result<&[u8]> {
    let first = *bytes.first().ok_or(Error::Malformed("empty vector"))?;
    let (len, hdr) = match first >> 6 {
        0 => (first as usize & 0x3f, 1),
        1 if bytes.len() >= 2 => ((u16::from_be_bytes([bytes[0], bytes[1]]) & 0x3fff) as usize, 2),
        2 if bytes.len() >= 4 => ((u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) & 0x3fff_ffff) as usize, 4),
        _ => return Err(Error::Malformed("vector length")),
    };
    bytes.get(hdr..hdr + len).ok_or(Error::Malformed("vector truncated"))
}

pub struct OpenMlsSession {
    provider: OpenMlsRustCrypto,
    signer: SignatureKeyPair,
    user_id: u64,
    group_id: u64,
    credential: CredentialWithKey,
    external_sender: Option<ExternalSender>,
    pending: Option<MlsGroup>,
    group: Option<MlsGroup>,
    our_commit: Option<Vec<u8>>,
}

impl OpenMlsSession {
    pub fn new(user_id: u64, group_id: u64) -> Result<Self> {
        let provider = OpenMlsRustCrypto::default();
        let signer = SignatureKeyPair::new(CIPHERSUITE.signature_algorithm()).map_err(mls("signature key"))?;
        signer.store(provider.storage()).map_err(mls("store signer"))?;
        let credential = CredentialWithKey {
            credential: BasicCredential::new(user_id.to_be_bytes().to_vec()).into(),
            signature_key: signer.public().into(),
        };
        Ok(Self { provider, signer, user_id, group_id, credential, external_sender: None, pending: None, group: None, our_commit: None })
    }

    fn capabilities() -> Capabilities {
        Capabilities::new(Some(&[ProtocolVersion::Mls10]), Some(&[CIPHERSUITE]), Some(&[]), Some(&[]), Some(&[CredentialType::Basic]))
    }

    fn join_config() -> MlsGroupJoinConfig {
        MlsGroupJoinConfig::builder()
            .use_ratchet_tree_extension(true)
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .max_past_epochs(0)
            .build()
    }

    fn drop_groups(&mut self) {
        for g in [self.pending.take(), self.group.take()].into_iter().flatten() {
            let mut g = g;
            let _ = g.delete(self.provider.storage());
        }
        self.our_commit = None;
    }

    fn create_pending_group(&mut self) -> Result<()> {
        let sender = self.external_sender.clone().ok_or(Error::Mls("no external sender".into()))?;
        if let Some(mut g) = self.pending.take() {
            let _ = g.delete(self.provider.storage());
        }
        let ext = Extensions::single(Extension::ExternalSenders(vec![sender])).map_err(mls("extensions"))?;
        let cfg = MlsGroupCreateConfig::builder()
            .ciphersuite(CIPHERSUITE)
            .capabilities(Self::capabilities())
            .use_ratchet_tree_extension(true)
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .max_past_epochs(0)
            .with_group_context_extensions(ext)
            .build();
        let g = MlsGroup::new_with_group_id(
            &self.provider,
            &self.signer,
            &cfg,
            GroupId::from_slice(&self.group_id.to_be_bytes()),
            self.credential.clone(),
        )
        .map_err(mls("create group"))?;
        self.pending = Some(g);
        Ok(())
    }
}

impl MlsSession for OpenMlsSession {
    fn protocol_version(&self) -> u16 {
        1
    }

    fn reset(&mut self) {
        self.drop_groups();
        if self.external_sender.is_some() {
            let _ = self.create_pending_group();
        }
    }

    fn set_external_sender(&mut self, package: &[u8]) -> Result<()> {
        let s = ExternalSender::tls_deserialize_exact(package).map_err(mls("external sender"))?;
        self.external_sender = Some(s);
        if self.group.is_none() {
            self.create_pending_group()?;
        }
        Ok(())
    }

    fn key_package(&mut self) -> Result<Vec<u8>> {
        let kp = KeyPackage::builder()
            .leaf_node_capabilities(Self::capabilities())
            .key_package_lifetime(key_package_lifetime())
            .build(CIPHERSUITE, &self.provider, &self.signer, self.credential.clone())
            .map_err(mls("key package"))?;
        let bytes = kp.key_package().tls_serialize_detached().map_err(mls("serialize key package"))?;
        tracing::debug!(user = self.user_id, bytes = bytes.len(), "dave: key package built");
        Ok(bytes)
    }

    fn process_proposals(&mut self, payload: &[u8], recognized_user_ids: &[u64]) -> Result<Option<CommitWelcome>> {
        let (&op, rest) = payload.split_first().ok_or(Error::Malformed("empty proposals"))?;
        let body = read_vl(rest)?;
        let mut recognized = recognized_user_ids.to_vec();
        recognized.push(self.user_id);
        let mut msgs = Vec::new();
        let mut refs = Vec::new();
        let mut r = body;
        while !r.is_empty() {
            match op {
                0 => msgs.push(MlsMessageIn::tls_deserialize(&mut r).map_err(mls("proposal message"))?),
                1 => refs.push(openmls::prelude::hash_ref::ProposalRef::tls_deserialize(&mut r).map_err(mls("proposal ref"))?),
                _ => return Err(Error::Protocol(format!("unknown proposals op {op}"))),
            }
        }
        self.our_commit = None;
        let provider = &self.provider;
        let group: &mut MlsGroup = match (&mut self.group, &mut self.pending) {
            (Some(g), _) => g,
            (None, Some(p)) => p,
            (None, None) => {
                return Err(Error::Mls("proposals before external sender".into()));
            }
        };
        if group.pending_commit().is_some() {
            group.clear_pending_commit(provider.storage()).map_err(mls("clear commit"))?;
        }
        for m in msgs {
            let pm = m.try_into_protocol_message().map_err(mls("proposal framing"))?;
            let processed = group.process_message(provider, pm).map_err(mls("process proposal"))?;
            match processed.into_content() {
                ProcessedMessageContent::ProposalMessage(qp) => {
                    if !matches!(qp.sender(), Sender::External(_)) {
                        return Err(Error::Mls("proposal not from external sender".into()));
                    }
                    if let Proposal::Add(add) = qp.proposal() {
                        let uid = credential_user_id(add.key_package().leaf_node().credential())
                            .ok_or(Error::Mls("add: bad credential".into()))?;
                        if !recognized.contains(&uid) {
                            return Err(Error::Mls(format!("add for unrecognized user {uid}")));
                        }
                    } else if !matches!(qp.proposal(), Proposal::Remove(_)) {
                        return Err(Error::Mls("unexpected proposal type".into()));
                    }
                    group.store_pending_proposal(provider.storage(), *qp).map_err(mls("store proposal"))?;
                }
                _ => return Err(Error::Mls("expected a proposal".into())),
            }
        }
        for r in refs {
            let _ = group.remove_pending_proposal(provider.storage(), &r);
        }
        if !group.has_pending_proposals() {
            return Ok(None);
        }
        let (commit, welcome, _) = group.commit_to_pending_proposals(provider, &self.signer).map_err(mls("commit"))?;
        let commit = commit.to_bytes().map_err(mls("serialize commit"))?;
        let welcome = welcome.map(|w| w.to_bytes()).transpose().map_err(mls("serialize welcome"))?;
        self.our_commit = Some(commit.clone());
        Ok(Some(CommitWelcome { commit, welcome }))
    }

    fn process_commit(&mut self, _transition_id: u16, commit: &[u8]) -> Result<()> {
        let ours = self.our_commit.take().is_some_and(|c| c == commit);
        if ours {
            let g = match self.group.as_mut() {
                Some(g) => g,
                None => {
                    let p = self.pending.take().ok_or(Error::Mls("no pending group".into()))?;
                    self.group.insert(p)
                }
            };
            g.merge_pending_commit(&self.provider).map_err(mls("merge own commit"))?;
            tracing::debug!(epoch = g.epoch().as_u64(), "dave: own commit merged");
            return Ok(());
        }
        let Some(g) = self.group.as_mut() else {
            return Err(Error::Mls("commit received before joining".into()));
        };
        if g.pending_commit().is_some() {
            g.clear_pending_commit(self.provider.storage()).map_err(mls("clear commit"))?;
        }
        let msg = MlsMessageIn::tls_deserialize_exact(commit).map_err(mls("commit message"))?;
        let pm = msg.try_into_protocol_message().map_err(mls("commit framing"))?;
        let processed = g.process_message(&self.provider, pm).map_err(mls("process commit"))?;
        match processed.into_content() {
            ProcessedMessageContent::StagedCommitMessage(sc) => {
                let removed = sc.self_removed();
                g.merge_staged_commit(&self.provider, *sc).map_err(mls("merge commit"))?;
                tracing::debug!(epoch = g.epoch().as_u64(), "dave: commit merged");
                if removed {
                    self.reset();
                    return Err(Error::Mls("removed from group".into()));
                }
                Ok(())
            }
            _ => Err(Error::Mls("expected a commit".into())),
        }
    }

    fn process_welcome(&mut self, _transition_id: u16, welcome: &[u8], recognized_user_ids: &[u64]) -> Result<()> {
        let w = match MlsMessageIn::tls_deserialize_exact(welcome).ok().and_then(|m| match m.extract() { MlsMessageBodyIn::Welcome(w) => Some(w), _ => None }) {
            Some(w) => w,
            None => Welcome::tls_deserialize_exact(welcome).map_err(mls("welcome"))?,
        };
        self.drop_groups();
        let staged = StagedWelcome::new_from_welcome(&self.provider, &Self::join_config(), w, None).map_err(mls("stage welcome"))?;
        if let Some(es) = &self.external_sender {
            let ok = staged.group_context().extensions().external_senders().is_some_and(|v| v.len() == 1 && &v[0] == es);
            if !ok {
                return Err(Error::Mls("welcome external sender mismatch".into()));
            }
        }
        for m in staged.members() {
            let uid = credential_user_id(&m.credential).ok_or(Error::Mls("member credential".into()))?;
            if uid != self.user_id && !recognized_user_ids.contains(&uid) {
                return Err(Error::Mls(format!("welcome contains unrecognized user {uid}")));
            }
        }
        let g = staged.into_group(&self.provider).map_err(mls("join group"))?;
        tracing::info!(epoch = g.epoch().as_u64(), members = ?g.members().filter_map(|m| credential_user_id(&m.credential)).collect::<Vec<_>>(), "dave: joined the group from a welcome");
        self.group = Some(g);
        Ok(())
    }

    fn export_sender_secret(&self, user_id: u64) -> Result<[u8; 16]> {
        let g = self.group.as_ref().ok_or(Error::Mls("not in group".into()))?;
        let v = g
            .export_secret(self.provider.crypto(), super::EXPORTER_LABEL, &user_id.to_le_bytes(), 16)
            .map_err(mls("export"))?;
        v.try_into().map_err(|_| Error::Mls("export length".into()))
    }

    fn epoch_authenticator(&self) -> Result<Vec<u8>> {
        let g = self.group.as_ref().ok_or(Error::Mls("not in group".into()))?;
        Ok(g.epoch_authenticator().as_slice().to_vec())
    }

    fn is_member(&self) -> bool {
        self.group.is_some()
    }

    fn member_ids(&self) -> Vec<u64> {
        self.group.as_ref().map(|g| g.members().filter_map(|m| credential_user_id(&m.credential)).collect()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Server {
        provider: OpenMlsRustCrypto,
        signer: SignatureKeyPair,
    }

    impl Server {
        fn new() -> Self {
            let provider = OpenMlsRustCrypto::default();
            let signer = SignatureKeyPair::new(CIPHERSUITE.signature_algorithm()).unwrap();
            Self { provider, signer }
        }
        fn external_sender(&self) -> Vec<u8> {
            ExternalSender::new(self.signer.public().into(), BasicCredential::new(b"voice".to_vec()).into())
                .tls_serialize_detached()
                .unwrap()
        }
        fn add(&self, kp: &[u8], epoch: u64) -> Vec<u8> {
            let kp = KeyPackageIn::tls_deserialize_exact(kp)
                .unwrap()
                .validate(self.provider.crypto(), ProtocolVersion::Mls10)
                .unwrap();
            ExternalProposal::new_add::<OpenMlsRustCrypto>(
                kp,
                GroupId::from_slice(&42u64.to_be_bytes()),
                GroupEpoch::from(epoch),
                &self.signer,
                SenderExtensionIndex::new(0),
            )
            .unwrap()
            .to_bytes()
            .unwrap()
        }
    }

    fn proposals(op: u8, msgs: &[Vec<u8>]) -> Vec<u8> {
        let inner: Vec<u8> = msgs.concat();
        let mut v = vec![op];
        assert!(inner.len() < 16384);
        if inner.len() < 64 {
            v.push(inner.len() as u8);
        } else {
            v.extend_from_slice(&((inner.len() as u16) | 0x4000).to_be_bytes());
        }
        v.extend_from_slice(&inner);
        v
    }

    fn welcome_only(cw: &CommitWelcome) -> Vec<u8> {
        cw.welcome.clone().unwrap()
    }

    fn assert_same_keys(a: &OpenMlsSession, b: &OpenMlsSession, users: &[u64]) {
        assert_eq!(a.epoch_authenticator().unwrap(), b.epoch_authenticator().unwrap());
        for u in users {
            assert_eq!(a.export_sender_secret(*u).unwrap(), b.export_sender_secret(*u).unwrap());
        }
    }

    #[test]
    fn key_package_has_the_lifetime_the_voice_gateway_requires() {
        let mut a = OpenMlsSession::new(7, 42).unwrap();
        let kp = KeyPackageIn::tls_deserialize_exact(a.key_package().unwrap())
            .unwrap()
            .validate(a.provider.crypto(), ProtocolVersion::Mls10)
            .unwrap();
        assert_eq!((kp.life_time().not_before(), kp.life_time().not_after()), (0, u64::MAX));
        assert_eq!(credential_user_id(kp.leaf_node().credential()), Some(7));
        assert_eq!(kp.ciphersuite(), CIPHERSUITE);
        assert_eq!(kp.extensions().iter().count(), 0);
        assert_eq!(kp.leaf_node().extensions().iter().count(), 0);
    }

    #[test]
    fn create_join_and_grow_group() {
        let server = Server::new();
        let es = server.external_sender();
        let (ua, ub, uc) = (1u64, 2u64, 3u64);
        let mut a = OpenMlsSession::new(ua, 42).unwrap();
        let mut b = OpenMlsSession::new(ub, 42).unwrap();
        let mut c = OpenMlsSession::new(uc, 42).unwrap();
        for s in [&mut a, &mut b, &mut c] {
            s.set_external_sender(&es).unwrap();
        }
        let kb = b.key_package().unwrap();
        let kc = c.key_package().unwrap();

        let p = proposals(0, &[server.add(&kb, 0)]);
        let cw = a.process_proposals(&p, &[ub]).unwrap().unwrap();
        assert!(b.process_proposals(&proposals(0, &[server.add(&kc, 0)]), &[]).is_err());
        a.process_commit(1, &cw.commit).unwrap();
        b.reset();
        b.process_welcome(1, &welcome_only(&cw), &[ua]).unwrap();
        assert_same_keys(&a, &b, &[ua, ub]);
        let mut ids = b.member_ids();
        ids.sort();
        assert_eq!(ids, vec![ua, ub]);

        let p = proposals(0, &[server.add(&kc, 1)]);
        let _cwa = a.process_proposals(&p, &[ub, uc]).unwrap().unwrap();
        let cwb = b.process_proposals(&p, &[ua, uc]).unwrap().unwrap();
        b.process_commit(2, &cwb.commit).unwrap();
        a.process_commit(2, &cwb.commit).unwrap();
        c.process_welcome(2, &welcome_only(&cwb), &[ua, ub]).unwrap();
        assert_same_keys(&a, &b, &[ua, ub, uc]);
        assert_same_keys(&a, &c, &[ua, ub, uc]);
        assert_ne!(a.export_sender_secret(ua).unwrap(), a.export_sender_secret(ub).unwrap());
    }
}
