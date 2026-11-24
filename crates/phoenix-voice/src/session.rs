use std::collections::HashMap;
use std::sync::Arc;

use crate::codec::{self, Codec, LocalCapabilities, VideoEncoderConfig};
use crate::dave::frame::FrameEncryptor;
use crate::media::{VideoEncoder, VideoEncoderFactory, VideoFrame};
use crate::rtp::av1::{self, Av1Packetizer};
use crate::rtp::h264::H264Packetizer;
use crate::rtp::rtcp::Feedback;
use crate::rtp::vp8::Vp8Packetizer;
use crate::rtp::{Packetizer, RtpSender, DEFAULT_MAX_PAYLOAD};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamKind {
    Camera,
    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSsrcs {
    pub ssrc: u32,
    pub rtx_ssrc: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamParams {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    CodecChanged { kind: StreamKind, from: Codec, to: Codec, reason: String },
    StreamFailed { kind: StreamKind, reason: String },
}

enum AnyPacketizer {
    H264(H264Packetizer),
    Vp8(Vp8Packetizer),
    Av1(Av1Packetizer),
}

impl AnyPacketizer {
    fn for_codec(c: Codec) -> Result<Self> {
        Ok(match c {
            Codec::H264 => Self::H264(H264Packetizer::new()),
            Codec::Vp8 => Self::Vp8(Vp8Packetizer::default()),
            Codec::Av1 => Self::Av1(Av1Packetizer),
            other => return Err(Error::Unsupported(format!("no send packetizer for {}", other.name()))),
        })
    }
}

struct VideoStream {
    ssrcs: StreamSsrcs,
    params: StreamParams,
    config: VideoEncoderConfig,
    encoder: Box<dyn VideoEncoder>,
    packetizer: AnyPacketizer,
    rtp: RtpSender,
    keyframe_pending: bool,
    dave: Option<FrameEncryptor>,
}

#[derive(Debug, Default)]
pub struct EncodeOutput {
    pub packets: Vec<Vec<u8>>,
    pub events: Vec<StreamEvent>,
}

pub struct VideoSessionManager {
    caps: LocalCapabilities,
    remote_accepts: Vec<Codec>,
    factory: Arc<dyn VideoEncoderFactory>,
    streams: HashMap<StreamKind, VideoStream>,
    total_bitrate_bps: u32,
    pub max_payload: usize,
}

impl VideoSessionManager {
    pub fn new(
        caps: LocalCapabilities,
        remote_accepts: Vec<Codec>,
        factory: Arc<dyn VideoEncoderFactory>,
        total_bitrate_bps: u32,
    ) -> Self {
        Self { caps, remote_accepts, factory, streams: HashMap::new(), total_bitrate_bps, max_payload: DEFAULT_MAX_PAYLOAD }
    }

    pub fn codec(&self, kind: StreamKind) -> Option<Codec> {
        self.streams.get(&kind).map(|s| s.config.codec)
    }

    pub fn ssrcs(&self, kind: StreamKind) -> Option<StreamSsrcs> {
        self.streams.get(&kind).map(|s| s.ssrcs)
    }

    pub fn is_active(&self, kind: StreamKind) -> bool {
        self.streams.contains_key(&kind)
    }

    fn budget_for(&self, kind: StreamKind, with: Option<StreamKind>) -> u32 {
        let cam = self.streams.contains_key(&StreamKind::Camera) || with == Some(StreamKind::Camera);
        let scr = self.streams.contains_key(&StreamKind::Screen) || with == Some(StreamKind::Screen);
        let (c, s) = codec::split_bitrate(self.total_bitrate_bps, cam, scr);
        match kind {
            StreamKind::Camera => c,
            StreamKind::Screen => s,
        }
    }

    fn open_encoder(
        &self,
        kind: StreamKind,
        params: StreamParams,
        bitrate: u32,
        first: Option<Codec>,
    ) -> Result<(VideoEncoderConfig, Box<dyn VideoEncoder>)> {
        let mut candidate = first;
        let mut last_err = Error::NoCodecAvailable;
        while let Some(c) = candidate {
            if c.sendable() {
                let cfg = VideoEncoderConfig::new(c, params.width, params.height, params.fps, bitrate, kind == StreamKind::Screen);
                match self.factory.create(&cfg) {
                    Ok(enc) => return Ok((cfg, enc)),
                    Err(e) => {
                        tracing::warn!(?kind, codec = c.name(), error = %e, "encoder unavailable, trying next codec");
                        last_err = e;
                    }
                }
            }
            candidate = codec::fallback_after(&self.caps, &self.remote_accepts, c);
        }
        Err(match last_err {
            Error::NoCodecAvailable => Error::NoCodecAvailable,
            e => Error::Encoder(format!("no usable codec: {e}")),
        })
    }

    pub fn start(&mut self, kind: StreamKind, ssrcs: StreamSsrcs, params: StreamParams) -> Result<Codec> {
        if self.streams.values().any(|s| s.ssrcs.ssrc == ssrcs.ssrc) || ssrcs.ssrc == ssrcs.rtx_ssrc {
            return Err(Error::Protocol("SSRC reused across streams".into()));
        }
        let first = codec::select_send_codec(&self.caps, &self.remote_accepts);
        let bitrate = self.budget_for(kind, Some(kind));
        let (config, encoder) = self.open_encoder(kind, params, bitrate, first)?;
        let codec = config.codec;
        let stream = VideoStream {
            ssrcs,
            params,
            packetizer: AnyPacketizer::for_codec(codec)?,
            rtp: RtpSender::new(ssrcs.ssrc, codec.payload_type()),
            config,
            encoder,
            keyframe_pending: true,
            dave: None,
        };
        self.streams.insert(kind, stream);
        self.rebalance();
        Ok(codec)
    }

    pub fn stop(&mut self, kind: StreamKind) {
        self.streams.remove(&kind);
        self.rebalance();
    }

    pub fn set_dave(&mut self, kind: StreamKind, enc: Option<FrameEncryptor>) {
        if let Some(s) = self.streams.get_mut(&kind) {
            s.dave = enc;
        }
    }

    pub fn set_total_bitrate(&mut self, bps: u32) {
        self.total_bitrate_bps = bps;
        self.rebalance();
    }

    fn rebalance(&mut self) {
        let (c, s) = codec::split_bitrate(
            self.total_bitrate_bps,
            self.streams.contains_key(&StreamKind::Camera),
            self.streams.contains_key(&StreamKind::Screen),
        );
        for (kind, st) in self.streams.iter_mut() {
            let b = if *kind == StreamKind::Camera { c } else { s };
            st.config.bitrate_bps = b;
            if let Err(e) = st.encoder.set_bitrate(b) {
                tracing::warn!(?kind, error = %e, "set_bitrate failed");
            }
        }
    }

    pub fn request_keyframe(&mut self, kind: StreamKind) {
        if let Some(s) = self.streams.get_mut(&kind) {
            s.keyframe_pending = true;
        }
    }

    pub fn handle_feedback(&mut self, fb: &Feedback) {
        if let Feedback::Pli { media_ssrc } = fb {
            for s in self.streams.values_mut() {
                if s.ssrcs.ssrc == *media_ssrc {
                    s.keyframe_pending = true;
                }
            }
        }
    }

    fn fallback(&mut self, kind: StreamKind, reason: String) -> Result<StreamEvent> {
        let (from, params, bitrate) = {
            let s = self.streams.get(&kind).ok_or(Error::Protocol("stream not active".into()))?;
            (s.config.codec, s.params, s.config.bitrate_bps)
        };
        let next = codec::fallback_after(&self.caps, &self.remote_accepts, from);
        match self.open_encoder(kind, params, bitrate, next) {
            Ok((config, encoder)) => {
                let to = config.codec;
                let packetizer = AnyPacketizer::for_codec(to)?;
                let s = self.streams.get_mut(&kind).ok_or(Error::Protocol("stream not active".into()))?;
                s.config = config;
                s.encoder = encoder;
                s.packetizer = packetizer;
                s.rtp.payload_type = to.payload_type();
                s.keyframe_pending = true;
                tracing::warn!(?kind, from = from.name(), to = to.name(), %reason, "video encoder fallback");
                Ok(StreamEvent::CodecChanged { kind, from, to, reason })
            }
            Err(e) => {
                self.streams.remove(&kind);
                self.rebalance();
                Ok(StreamEvent::StreamFailed { kind, reason: format!("{reason}; {e}") })
            }
        }
    }

    pub fn encode(&mut self, kind: StreamKind, frame: &VideoFrame, rtp_timestamp: u32) -> Result<EncodeOutput> {
        let mut out = EncodeOutput::default();
        let max = self.max_payload;
        for _ in 0..4 {
            let Some(s) = self.streams.get_mut(&kind) else {
                return Ok(out);
            };
            let force = s.keyframe_pending;
            let attempt = s.encoder.encode(frame, force).and_then(|enc| match enc {
                None => Ok(Vec::new()),
                Some(e) if e.codec != s.config.codec => {
                    Err(Error::Encoder(format!("encoder produced {} for {} stream", e.codec.name(), s.config.codec.name())))
                }
                Some(e) => {
                    if e.keyframe {
                        s.keyframe_pending = false;
                    }
                    let payloads = packetize(s, &e.data, max)?;
                    Ok(s.rtp.wrap(payloads, rtp_timestamp))
                }
            });
            match attempt {
                Ok(p) => {
                    out.packets = p;
                    return Ok(out);
                }
                Err(e) => {
                    let ev = self.fallback(kind, e.to_string())?;
                    let failed = matches!(ev, StreamEvent::StreamFailed { .. });
                    out.events.push(ev);
                    if failed {
                        return Ok(out);
                    }
                }
            }
        }
        Ok(out)
    }
}

fn packetize(s: &mut VideoStream, data: &[u8], max: usize) -> Result<Vec<Vec<u8>>> {
    let codec = s.config.codec;
    match &mut s.packetizer {
        AnyPacketizer::H264(p) => {
            let au = p.prepare_access_unit(data);
            let au = match &mut s.dave {
                Some(d) => d.encrypt(codec, &au)?,
                None => au,
            };
            p.packetize_prepared(&au, max)
        }
        AnyPacketizer::Av1(p) => match &mut s.dave {
            Some(d) => {
                let f = av1::strip_last_obu_size(data)?;
                let f = d.encrypt(codec, &f)?;
                p.packetize(&f, max)
            }
            None => p.packetize(data, max),
        },
        AnyPacketizer::Vp8(p) => match &mut s.dave {
            Some(d) => p.packetize(&d.encrypt(codec, data)?, max),
            None => p.packetize(data, max),
        },
    }
}
