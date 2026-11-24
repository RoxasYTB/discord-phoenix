use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct JitterPacket {
    pub sequence: u16,
    pub timestamp: u32,
    pub marker: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum JitterOutput {
    Packet(JitterPacket),
    Lost { first: u16, count: u16 },
}

pub struct JitterBuffer {
    depth: usize,
    capacity: usize,
    packets: BTreeMap<u64, JitterPacket>,
    next: Option<u64>,
    highest: Option<u64>,
}

impl JitterBuffer {
    pub fn new(depth: usize, capacity: usize) -> Self {
        Self { depth: depth.max(1), capacity: capacity.max(depth + 1), packets: BTreeMap::new(), next: None, highest: None }
    }

    fn extend(&self, seq: u16) -> u64 {
        let Some(h) = self.highest else { return (1u64 << 32) + seq as u64 };
        let base = h & !0xffff;
        let cand = base | seq as u64;
        let delta = cand as i64 - h as i64;
        if delta > 0x8000 {
            cand.saturating_sub(0x10000)
        } else if delta < -0x8000 {
            cand + 0x10000
        } else {
            cand
        }
    }

    pub fn push(&mut self, p: JitterPacket) {
        let ext = self.extend(p.sequence);
        if self.next.is_some_and(|n| ext < n) {
            return;
        }
        if self.highest.is_none_or(|h| ext > h) {
            self.highest = Some(ext);
        }
        if self.next.is_none() {
            self.next = Some(ext);
        }
        if self.next.is_some_and(|n| ext < n) {
            return;
        }
        self.packets.entry(ext).or_insert(p);
        if self.next.is_some_and(|n| ext < n) {
            self.next = Some(ext);
        }
        while self.packets.len() > self.capacity {
            if let Some((&k, _)) = self.packets.iter().next() {
                self.packets.remove(&k);
                self.next = Some(k + 1);
            }
        }
    }

    pub fn pop(&mut self) -> Option<JitterOutput> {
        let next = self.next?;
        if let Some(p) = self.packets.remove(&next) {
            self.next = Some(next + 1);
            return Some(JitterOutput::Packet(p));
        }
        let (&first_avail, _) = self.packets.iter().next()?;
        if self.packets.len() >= self.depth || first_avail - next > self.depth as u64 {
            self.next = Some(first_avail);
            let count = (first_avail - next).min(u16::MAX as u64) as u16;
            return Some(JitterOutput::Lost { first: next as u16, count });
        }
        None
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(s: u16) -> JitterPacket {
        JitterPacket { sequence: s, timestamp: s as u32 * 960, marker: false, payload: vec![s as u8] }
    }

    fn drain(j: &mut JitterBuffer) -> Vec<String> {
        let mut v = Vec::new();
        while let Some(o) = j.pop() {
            v.push(match o {
                JitterOutput::Packet(p) => p.sequence.to_string(),
                JitterOutput::Lost { first, count } => format!("lost{first}x{count}"),
            });
        }
        v
    }

    #[test]
    fn reorders_across_wrap() {
        let mut j = JitterBuffer::new(3, 64);
        for s in [65534, 0, 65535, 1] {
            j.push(pk(s));
        }
        assert_eq!(drain(&mut j), ["65534", "65535", "0", "1"]);
    }

    #[test]
    fn declares_loss_after_depth() {
        let mut j = JitterBuffer::new(2, 64);
        j.push(pk(10));
        assert_eq!(drain(&mut j), ["10"]);
        j.push(pk(12));
        assert!(drain(&mut j).is_empty());
        j.push(pk(13));
        assert_eq!(drain(&mut j), ["lost11x1", "12", "13"]);
        j.push(pk(11));
        assert!(j.is_empty());
    }
}
