use std::sync::Arc;

use phoenix_voice::codec::{Codec, LocalCapabilities};
use phoenix_voice::crypto::{EncryptionMode, TransportCipher};
use phoenix_voice::dave::frame::{FrameDecryptor, FrameEncryptor};
use phoenix_voice::dave::ratchet::HashRatchet;
use phoenix_voice::media::mock::MockEncoderFactory;
use phoenix_voice::media::VideoFrame;
use phoenix_voice::rtp::h264::H264Depacketizer;
use phoenix_voice::rtp::rtcp::Feedback;
use phoenix_voice::rtp::{parse_packet, Depacketizer};
use phoenix_voice::session::{StreamEvent, StreamKind, StreamParams, StreamSsrcs, VideoSessionManager};

fn frame(i: u64) -> VideoFrame {
    VideoFrame { width: 1280, height: 720, pts_us: i * 33_333, data: Arc::from(vec![0u8; 16]) }
}

fn caps() -> LocalCapabilities {
    LocalCapabilities {
        hw_av1_encoder: true,
        h264_encoder: true,
        vp8_encoder: true,
        decoders: vec![Codec::H264, Codec::Vp8, Codec::Av1, Codec::H265],
    }
}

const ALL: [Codec; 4] = [Codec::H265, Codec::Av1, Codec::H264, Codec::Vp8];
const P: StreamParams = StreamParams { width: 1280, height: 720, fps: 30 };
const CAM: StreamSsrcs = StreamSsrcs { ssrc: 100, rtx_ssrc: 101 };
const SCR: StreamSsrcs = StreamSsrcs { ssrc: 200, rtx_ssrc: 201 };

#[test]
fn camera_and_screen_fallback_independently_without_panic() {
    let factory = MockEncoderFactory {
        unavailable: vec![],
        fail_after: vec![(Codec::Av1, 3), (Codec::H264, 5)],
        ..Default::default()
    };
    let mut m = VideoSessionManager::new(caps(), ALL.to_vec(), Arc::new(factory), 3_000_000);
    assert_eq!(m.start(StreamKind::Camera, CAM, P).unwrap(), Codec::Av1);
    assert_eq!(m.start(StreamKind::Screen, SCR, P).unwrap(), Codec::Av1);
    assert!(m.start(StreamKind::Screen, CAM, P).is_err(), "ssrc reuse refused");

    let mut events = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for i in 0..20 {
        for kind in [StreamKind::Camera, StreamKind::Screen] {
            let out = m.encode(kind, &frame(i), (i * 3000) as u32).unwrap();
            assert!(!out.packets.is_empty(), "frame {i} for {kind:?} produced packets");
            for p in &out.packets {
                let (h, _) = parse_packet(p).unwrap();
                let want = if kind == StreamKind::Camera { 100 } else { 200 };
                assert_eq!(h.ssrc, want);
                seen.insert((kind, h.payload_type));
                assert_ne!(h.payload_type, Codec::H265.payload_type());
            }
            events.extend(out.events);
        }
    }
    assert_eq!(m.codec(StreamKind::Camera), Some(Codec::Vp8));
    assert_eq!(m.codec(StreamKind::Screen), Some(Codec::Vp8));
    let changes: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::CodecChanged { kind, from, to, .. } => Some((*kind, *from, *to)),
            _ => None,
        })
        .collect();
    assert!(changes.contains(&(StreamKind::Camera, Codec::Av1, Codec::H264)));
    assert!(changes.contains(&(StreamKind::Camera, Codec::H264, Codec::Vp8)));
    assert!(changes.contains(&(StreamKind::Screen, Codec::H264, Codec::Vp8)));
    assert!(seen.contains(&(StreamKind::Screen, Codec::Vp8.payload_type())));
}

#[test]
fn all_codecs_fail_stops_only_that_stream() {
    let factory = MockEncoderFactory { fail_after: vec![(Codec::H264, 2)], unavailable: vec![Codec::Av1, Codec::Vp8], ..Default::default() };
    let mut m = VideoSessionManager::new(caps(), ALL.to_vec(), Arc::new(factory), 3_000_000);
    assert_eq!(m.start(StreamKind::Camera, CAM, P).unwrap(), Codec::H264);
    assert_eq!(m.start(StreamKind::Screen, SCR, P).unwrap(), Codec::H264);
    for i in 0..2 {
        assert!(m.encode(StreamKind::Camera, &frame(i), 0).unwrap().events.is_empty());
    }
    let out = m.encode(StreamKind::Camera, &frame(2), 0).unwrap();
    assert!(matches!(out.events.as_slice(), [StreamEvent::StreamFailed { kind: StreamKind::Camera, .. }]));
    assert!(!m.is_active(StreamKind::Camera));
    assert!(!m.encode(StreamKind::Screen, &frame(0), 0).unwrap().packets.is_empty());
    assert!(m.encode(StreamKind::Camera, &frame(3), 0).unwrap().packets.is_empty());
}

#[test]
fn h265_only_peer_means_no_video_not_h265() {
    let mut m = VideoSessionManager::new(caps(), vec![Codec::H265], Arc::new(MockEncoderFactory::default()), 1_000_000);
    assert!(m.start(StreamKind::Camera, CAM, P).is_err());
}

#[test]
fn pli_forces_keyframe_on_matching_stream_with_dave_and_transport() {
    let factory = MockEncoderFactory { unavailable: vec![Codec::Av1], ..Default::default() };
    let mut m = VideoSessionManager::new(caps(), ALL.to_vec(), Arc::new(factory), 3_000_000);
    m.start(StreamKind::Camera, CAM, P).unwrap();
    m.start(StreamKind::Screen, SCR, P).unwrap();
    m.set_dave(StreamKind::Camera, Some(FrameEncryptor::new(Box::new(HashRatchet::new(&[5; 16])))));
    let mut dec = FrameDecryptor::new(Box::new(HashRatchet::new(&[5; 16])));
    let mut tx = TransportCipher::new(EncryptionMode::Aes256GcmRtpSize, &[2; 32]).unwrap();
    let rx = TransportCipher::new(EncryptionMode::Aes256GcmRtpSize, &[2; 32]).unwrap();

    let mut decode = |pkts: &[Vec<u8>]| -> Vec<u8> {
        let mut d = H264Depacketizer::default();
        let mut au = None;
        for p in pkts {
            let wire = tx.encrypt_rtp(p).unwrap();
            let plain = rx.decrypt_rtp(&wire).unwrap();
            let (h, pl) = parse_packet(&plain).unwrap();
            au = d.push(pl, h.marker).unwrap();
        }
        dec.decrypt(&au.unwrap()).unwrap()
    };

    let first = m.encode(StreamKind::Camera, &frame(0), 0).unwrap();
    let au = decode(&first.packets);
    assert!(phoenix_voice::rtp::h264::is_keyframe(&au));
    assert_eq!(&au[..5], &[0, 0, 0, 1, 0x67]);
    let delta = m.encode(StreamKind::Camera, &frame(1), 3000).unwrap();
    assert!(!phoenix_voice::rtp::h264::is_keyframe(&decode(&delta.packets)));
    m.handle_feedback(&Feedback::Pli { media_ssrc: 100 });
    let k = m.encode(StreamKind::Camera, &frame(2), 6000).unwrap();
    let au = decode(&k.packets);
    assert!(phoenix_voice::rtp::h264::is_keyframe(&au));
    assert_eq!(&au[..5], &[0, 0, 0, 1, 0x67]);
}
