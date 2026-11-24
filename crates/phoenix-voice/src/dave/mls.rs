use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitWelcome {
    pub commit: Vec<u8>,
    pub welcome: Option<Vec<u8>>,
}

pub trait MlsSession: Send {
    fn protocol_version(&self) -> u16;
    fn reset(&mut self);
    fn set_external_sender(&mut self, package: &[u8]) -> Result<()>;
    fn key_package(&mut self) -> Result<Vec<u8>>;
    fn process_proposals(&mut self, payload: &[u8], recognized_user_ids: &[u64]) -> Result<Option<CommitWelcome>>;
    fn process_commit(&mut self, transition_id: u16, commit: &[u8]) -> Result<()>;
    fn process_welcome(&mut self, transition_id: u16, welcome: &[u8], recognized_user_ids: &[u64]) -> Result<()>;
    fn export_sender_secret(&self, user_id: u64) -> Result<[u8; 16]>;
    fn epoch_authenticator(&self) -> Result<Vec<u8>>;
    fn is_member(&self) -> bool {
        self.epoch_authenticator().is_ok()
    }
    fn member_ids(&self) -> Vec<u64> {
        Vec::new()
    }
}

pub fn sender_ratchet(mls: &dyn MlsSession, user_id: u64) -> Result<super::ratchet::HashRatchet> {
    Ok(super::ratchet::HashRatchet::new(&mls.export_sender_secret(user_id)?))
}
