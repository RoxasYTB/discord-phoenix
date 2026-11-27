use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use super::frame::{FrameDecryptor, FrameEncryptor, is_encrypted};
use super::mls::{CommitWelcome, MlsSession, sender_ratchet};
use crate::codec::Codec;
use crate::gateway::DaveServerMessage;
use crate::Result;

pub const KEY_RETENTION: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveAction {
    KeyPackage(Vec<u8>),
    CommitWelcome(CommitWelcome),
    TransitionReady(u16),
    InvalidCommitWelcome(u16),
}

struct UserDecryptor {
    current: FrameDecryptor,
    previous: Option<(FrameDecryptor, Instant)>,
}

fn dave_message_name(m: &DaveServerMessage) -> String {
    match m {
        DaveServerMessage::PrepareTransition { transition_id, protocol_version } => format!("op21 prepare_transition id={transition_id} v={protocol_version}"),
        DaveServerMessage::ExecuteTransition { transition_id } => format!("op22 execute_transition id={transition_id}"),
        DaveServerMessage::PrepareEpoch { epoch, protocol_version } => format!("op24 prepare_epoch epoch={epoch} v={protocol_version}"),
        DaveServerMessage::ExternalSender(p) => format!("op25 external_sender {} bytes", p.len()),
        DaveServerMessage::Proposals(p) => format!("op27 proposals {} bytes", p.len()),
        DaveServerMessage::AnnounceCommit { transition_id, commit } => format!("op29 commit id={transition_id} {} bytes", commit.len()),
        DaveServerMessage::Welcome { transition_id, welcome } => format!("op30 welcome id={transition_id} {} bytes", welcome.len()),
    }
}

pub struct DaveSession {
    mls: Box<dyn MlsSession>,
    self_id: u64,
    recognized: HashSet<u64>,
    active_version: u16,
    negotiated_version: u16,
    pending: HashMap<u16, u16>,
    encryptor: Option<FrameEncryptor>,
    decryptors: HashMap<u64, UserDecryptor>,
    passthrough_until: Option<Instant>,
}

impl DaveSession {
    pub fn new(mls: Box<dyn MlsSession>, self_id: u64) -> Self {
        Self {
            mls,
            self_id,
            recognized: HashSet::new(),
            active_version: 0,
            negotiated_version: 0,
            pending: HashMap::new(),
            encryptor: None,
            decryptors: HashMap::new(),
            passthrough_until: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_version > 0 && self.encryptor.is_some()
    }

    pub fn protocol_version(&self) -> u16 {
        self.active_version
    }

    pub fn epoch_authenticator(&self) -> Option<Vec<u8>> {
        self.mls.epoch_authenticator().ok()
    }

    pub fn on_session_description(&mut self, dave_protocol_version: u16) -> Vec<DaveAction> {
        self.negotiated_version = dave_protocol_version;
        if dave_protocol_version == 0 {
            return Vec::new();
        }
        self.key_package_action()
    }

    pub fn add_users(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.recognized.extend(ids);
    }

    pub fn remove_user(&mut self, id: u64) {
        self.recognized.remove(&id);
        self.decryptors.remove(&id);
    }

    fn key_package_action(&mut self) -> Vec<DaveAction> {
        match self.mls.key_package() {
            Ok(kp) => {
                tracing::debug!(bytes = kp.len(), "dave: sending key package");
                vec![DaveAction::KeyPackage(kp)]
            }
            Err(e) => {
                tracing::warn!(error = %e, "dave: key package");
                Vec::new()
            }
        }
    }

    fn recognized_vec(&self) -> Vec<u64> {
        self.recognized.iter().copied().filter(|u| *u != self.self_id).collect()
    }

    fn passthrough_allowed(&self) -> bool {
        self.active_version == 0 || self.passthrough_until.is_some_and(|t| Instant::now() < t)
    }

    fn install_keys(&mut self) {
        let ratchet = match sender_ratchet(self.mls.as_ref(), self.self_id) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, member = self.mls.is_member(), "dave: no sender key, staying in passthrough");
                return;
            }
        };
        match &mut self.encryptor {
            Some(enc) => enc.set_ratchet(Box::new(ratchet)),
            None => self.encryptor = Some(FrameEncryptor::new(Box::new(ratchet))),
        }
        let now = Instant::now();
        let members: Vec<u64> = {
            let m = self.mls.member_ids();
            if m.is_empty() { self.recognized.iter().copied().collect() } else { m }
        };
        for uid in members.into_iter().filter(|u| *u != self.self_id) {
            let Ok(r) = sender_ratchet(self.mls.as_ref(), uid) else { continue };
            let fresh = FrameDecryptor::new(Box::new(r));
            match self.decryptors.remove(&uid) {
                Some(old) => {
                    self.decryptors.insert(uid, UserDecryptor { current: fresh, previous: Some((old.current, now + KEY_RETENTION)) });
                }
                None => {
                    self.decryptors.insert(uid, UserDecryptor { current: fresh, previous: None });
                }
            }
        }
        self.passthrough_until = Some(now + KEY_RETENTION);
    }

    fn execute(&mut self, version: u16) {
        if version == 0 {
            self.active_version = 0;
            self.encryptor = None;
            self.decryptors.clear();
            self.passthrough_until = None;
        } else {
            self.active_version = version;
            self.install_keys();
        }
        tracing::info!(version, keys = ?self.key_users(), encryptor = self.encryptor.is_some(), "dave: transition executed");
    }

    fn stage(&mut self, transition_id: u16, version: u16, out: &mut Vec<DaveAction>) {
        if transition_id == 0 {
            self.execute(version);
        } else {
            self.pending.insert(transition_id, version);
            out.push(DaveAction::TransitionReady(transition_id));
        }
    }

    fn fail(&mut self, transition_id: u16, out: &mut Vec<DaveAction>) {
        out.push(DaveAction::InvalidCommitWelcome(transition_id));
        self.mls.reset();
        out.extend(self.key_package_action());
    }

    pub fn key_users(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self.decryptors.keys().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn handle(&mut self, msg: &DaveServerMessage) -> Vec<DaveAction> {
        tracing::debug!(message = %dave_message_name(msg), member = self.mls.is_member(), version = self.active_version, "dave: gateway message");
        let mut out = Vec::new();
        match msg {
            DaveServerMessage::PrepareTransition { transition_id, protocol_version } => {
                self.stage(*transition_id, *protocol_version, &mut out);
            }
            DaveServerMessage::ExecuteTransition { transition_id } => {
                if let Some(v) = self.pending.remove(transition_id) {
                    self.execute(v);
                } else {
                    tracing::debug!(transition_id, "dave: execute for unknown transition");
                }
            }
            DaveServerMessage::PrepareEpoch { epoch, protocol_version } => {
                if *protocol_version > 0 {
                    self.negotiated_version = *protocol_version;
                }
                if *epoch == 1 {
                    self.mls.reset();
                    out.extend(self.key_package_action());
                }
            }
            DaveServerMessage::ExternalSender(pkg) => {
                if let Err(e) = self.mls.set_external_sender(pkg) {
                    tracing::warn!(error = %e, "dave: external sender");
                }
            }
            DaveServerMessage::Proposals(p) => match self.mls.process_proposals(p, &self.recognized_vec()) {
                Ok(Some(cw)) => out.push(DaveAction::CommitWelcome(cw)),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "dave: proposals rejected"),
            },
            DaveServerMessage::AnnounceCommit { transition_id, commit } => {
                let was_member = self.mls.is_member();
                match self.mls.process_commit(*transition_id, commit) {
                    Ok(()) => {
                        let v = self.negotiated_version.max(1);
                        self.stage(*transition_id, v, &mut out);
                    }
                    Err(_) if !was_member => {}
                    Err(e) => {
                        tracing::warn!(error = %e, "dave: invalid commit");
                        self.fail(*transition_id, &mut out);
                    }
                }
            }
            DaveServerMessage::Welcome { transition_id, welcome } => {
                match self.mls.process_welcome(*transition_id, welcome, &self.recognized_vec()) {
                    Ok(()) => {
                        let v = self.negotiated_version.max(1);
                        self.stage(*transition_id, v, &mut out);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "dave: invalid welcome");
                        self.fail(*transition_id, &mut out);
                    }
                }
            }
        }
        out
    }

    pub fn encrypt(&mut self, codec: Codec, frame: &[u8]) -> Result<Vec<u8>> {
        match (&mut self.encryptor, self.active_version) {
            (Some(enc), v) if v > 0 => enc.encrypt(codec, frame),
            _ => Ok(frame.to_vec()),
        }
    }

    pub fn decrypt(&mut self, user_id: u64, frame: &[u8]) -> Result<Vec<u8>> {
        let passthrough = self.passthrough_allowed();
        if !is_encrypted(frame) {
            if passthrough || frame == super::frame::OPUS_SILENCE {
                return Ok(frame.to_vec());
            }
            return Err(crate::Error::Crypto("unencrypted frame while E2EE required"));
        }
        let Some(d) = self.decryptors.get_mut(&user_id) else {
            return Err(crate::Error::Crypto("no key for sender"));
        };
        match d.current.decrypt(frame) {
            Ok(f) => Ok(f),
            Err(e) => {
                if let Some((prev, until)) = &mut d.previous {
                    if Instant::now() < *until {
                        return prev.decrypt(frame);
                    }
                    d.previous = None;
                }
                Err(e)
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::dave::openmls_session::OpenMlsSession;

    #[test]
    fn passthrough_before_activation_and_downgrade() {
        let mut s = DaveSession::new(Box::new(OpenMlsSession::new(1, 9).unwrap()), 1);
        assert!(matches!(s.on_session_description(1)[..], [DaveAction::KeyPackage(_)]));
        assert_eq!(s.encrypt(Codec::Opus, &[1, 2, 3]).unwrap(), vec![1, 2, 3]);
        assert_eq!(s.decrypt(2, &[9, 9]).unwrap(), vec![9, 9]);
        let a = s.handle(&DaveServerMessage::PrepareTransition { transition_id: 5, protocol_version: 0 });
        assert_eq!(a, vec![DaveAction::TransitionReady(5)]);
        s.handle(&DaveServerMessage::ExecuteTransition { transition_id: 5 });
        assert!(!s.is_active());
    }

    mod harness {
        use crate::dave::openmls_session::OpenMlsSession;
        use crate::dave::session::{DaveAction, DaveSession};
        use crate::gateway::DaveServerMessage;
        use openmls::prelude::tls_codec::{Deserialize as _, Serialize as _};
        use openmls::prelude::*;
        use openmls_basic_credential::SignatureKeyPair;
        use openmls_rust_crypto::OpenMlsRustCrypto;

        pub(crate) struct VoiceServer {
            ss: SignatureKeyPair,
            sp: OpenMlsRustCrypto,
            es: Vec<u8>,
        }

        impl VoiceServer {
            pub(crate) fn new() -> Self {
                let cs = crate::dave::openmls_session::CIPHERSUITE;
                let ss = SignatureKeyPair::new(cs.signature_algorithm()).unwrap();
                let es = ExternalSender::new(ss.public().into(), BasicCredential::new(b"v".to_vec()).into()).tls_serialize_detached().unwrap();
                Self { ss, sp: OpenMlsRustCrypto::default(), es }
            }

            pub(crate) fn client(&self, id: u64) -> (DaveSession, Vec<u8>) {
                let mut c = DaveSession::new(Box::new(OpenMlsSession::new(id, 9).unwrap()), id);
                c.handle(&DaveServerMessage::ExternalSender(self.es.clone()));
                let DaveAction::KeyPackage(kp) = c.on_session_description(1).remove(0) else { panic!() };
                (c, kp)
            }

            fn wrap(prop: Vec<u8>) -> Vec<u8> {
                let mut payload = vec![0u8];
                payload.extend_from_slice(&((prop.len() as u16) | 0x4000).to_be_bytes());
                payload.extend_from_slice(&prop);
                payload
            }

            pub(crate) fn add(&self, kp: &[u8], epoch: u64) -> Vec<u8> {
                let kp = KeyPackageIn::tls_deserialize_exact(kp).unwrap().validate(self.sp.crypto(), ProtocolVersion::Mls10).unwrap();
                let gid = GroupId::from_slice(&9u64.to_be_bytes());
                Self::wrap(ExternalProposal::new_add::<OpenMlsRustCrypto>(kp, gid, GroupEpoch::from(epoch), &self.ss, SenderExtensionIndex::new(0)).unwrap().to_bytes().unwrap())
            }

            pub(crate) fn remove(&self, leaf: u32, epoch: u64) -> Vec<u8> {
                let gid = GroupId::from_slice(&9u64.to_be_bytes());
                Self::wrap(
                    ExternalProposal::new_remove::<OpenMlsRustCrypto>(LeafNodeIndex::new(leaf), gid, GroupEpoch::from(epoch), &self.ss, SenderExtensionIndex::new(0))
                        .unwrap()
                        .to_bytes()
                        .unwrap(),
                )
            }
        }
    }

    use harness::VoiceServer;

    pub(crate) fn paired_sessions() -> (DaveSession, DaveSession) {
        let (a, b, _) = paired_with_server();
        (a, b)
    }

    fn paired_with_server() -> (DaveSession, DaveSession, VoiceServer) {
        let server = VoiceServer::new();
        let (mut a, _) = server.client(1);
        let (mut b, kb) = server.client(2);
        a.add_users([2]);
        b.add_users([1]);
        let DaveAction::CommitWelcome(cw) = a.handle(&DaveServerMessage::Proposals(server.add(&kb, 0))).remove(0) else { panic!() };
        assert_eq!(a.handle(&DaveServerMessage::AnnounceCommit { transition_id: 3, commit: cw.commit.clone() }), vec![DaveAction::TransitionReady(3)]);
        assert_eq!(b.handle(&DaveServerMessage::Welcome { transition_id: 3, welcome: cw.welcome.unwrap() }), vec![DaveAction::TransitionReady(3)]);
        assert!(!a.is_active());
        a.handle(&DaveServerMessage::ExecuteTransition { transition_id: 3 });
        b.handle(&DaveServerMessage::ExecuteTransition { transition_id: 3 });
        assert!(a.is_active() && b.is_active());
        (a, b, server)
    }

    #[test]
    fn a_third_member_joins_then_one_leaves() {
        let (mut a, mut b, server) = paired_with_server();
        let before = a.encrypt(Codec::Opus, &[0x78, 1, 1]).unwrap();
        let (mut c, kc) = server.client(3);
        a.add_users([3]);
        b.add_users([3]);
        c.add_users([1, 2]);
        let add = server.add(&kc, 1);
        let DaveAction::CommitWelcome(cw) = a.handle(&DaveServerMessage::Proposals(add.clone())).remove(0) else { panic!() };
        assert!(matches!(b.handle(&DaveServerMessage::Proposals(add))[..], [DaveAction::CommitWelcome(_)]));
        for s in [&mut a, &mut b] {
            assert_eq!(s.handle(&DaveServerMessage::AnnounceCommit { transition_id: 4, commit: cw.commit.clone() }), vec![DaveAction::TransitionReady(4)]);
        }
        assert_eq!(c.handle(&DaveServerMessage::Welcome { transition_id: 4, welcome: cw.welcome.unwrap() }), vec![DaveAction::TransitionReady(4)]);
        let in_flight = a.encrypt(Codec::Opus, &[0x78, 2, 2]).unwrap();
        for s in [&mut a, &mut b, &mut c] {
            s.handle(&DaveServerMessage::ExecuteTransition { transition_id: 4 });
            assert!(s.is_active());
        }
        assert_eq!(a.epoch_authenticator(), c.epoch_authenticator());
        assert_eq!(b.key_users(), vec![1, 3]);
        assert_eq!(b.decrypt(1, &before).unwrap(), [0x78, 1, 1]);
        assert_eq!(b.decrypt(1, &in_flight).unwrap(), [0x78, 2, 2]);
        let f = a.encrypt(Codec::Opus, &[0x78, 3, 3]).unwrap();
        assert_eq!(b.decrypt(1, &f).unwrap(), [0x78, 3, 3]);
        assert_eq!(c.decrypt(1, &f).unwrap(), [0x78, 3, 3]);
        let g = c.encrypt(Codec::Opus, &[0x78, 4, 4]).unwrap();
        assert_eq!(a.decrypt(3, &g).unwrap(), [0x78, 4, 4]);

        a.remove_user(2);
        c.remove_user(2);
        let rm = server.remove(1, 2);
        let DaveAction::CommitWelcome(cw) = c.handle(&DaveServerMessage::Proposals(rm.clone())).remove(0) else { panic!() };
        assert!(cw.welcome.is_none());
        a.handle(&DaveServerMessage::Proposals(rm));
        for s in [&mut a, &mut c] {
            assert_eq!(s.handle(&DaveServerMessage::AnnounceCommit { transition_id: 5, commit: cw.commit.clone() }), vec![DaveAction::TransitionReady(5)]);
            s.handle(&DaveServerMessage::ExecuteTransition { transition_id: 5 });
        }
        assert_eq!(a.key_users(), vec![3]);
        let h = a.encrypt(Codec::Opus, &[0x78, 5, 5]).unwrap();
        assert_eq!(c.decrypt(1, &h).unwrap(), [0x78, 5, 5]);
        assert!(b.decrypt(1, &h).is_err());
    }

    #[test]
    fn end_to_end_media_between_two_clients() {
        let (mut a, mut b) = paired_sessions();
        assert_eq!(a.epoch_authenticator(), b.epoch_authenticator());
        let frame = [0x78u8, 1, 2, 3, 4, 5, 6, 7];
        let enc = a.encrypt(Codec::Opus, &frame).unwrap();
        assert_ne!(&enc[..], &frame[..]);
        assert_eq!(b.decrypt(1, &enc).unwrap(), frame);
        assert!(b.decrypt(3, &enc).is_err());
    }
}
