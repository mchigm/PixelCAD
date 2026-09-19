//! Acceptance criterion: replaying `docs/samples/ship.pxc` twice yields
//! identical document hashes.

use pixelcad_core::{parse_script, Engine};

const SHIP_SCRIPT: &str = include_str!("../../../docs/samples/ship.pxc");

#[test]
fn replaying_ship_script_twice_yields_identical_document_hash() {
    let commands = parse_script(SHIP_SCRIPT).expect("ship.pxc must parse");

    let mut engine_a = Engine::new();
    engine_a.execute_all(commands.clone()).expect("ship.pxc must replay cleanly");

    let mut engine_b = Engine::new();
    engine_b.execute_all(commands).expect("ship.pxc must replay cleanly");

    let hash_a = engine_a.document_hash().expect("ship.pxc must create a canvas");
    let hash_b = engine_b.document_hash().expect("ship.pxc must create a canvas");

    assert_eq!(hash_a, hash_b);
}

/// Phase 1 regression guard for Phase 0's byte-identical PNG output.
///
/// `docs/samples/ship.pxc` is a single-layer script. When the multi-layer
/// `Document` landed in Phase 1, the *composited* output of that script had
/// to stay bit-identical to Phase 0's raw single-buffer output, or the
/// documented PNG sha256
/// `bde008684f8384379e33f514d15d4ca4c1aed97798d52a08bbb1cccd6f781232` would
/// have silently changed. This test pins the composited bytes so no future
/// change to compositing, layer defaults, or drawing can break that quietly:
/// the PNG encoder is a pure function of these bytes plus the dimensions.
#[test]
fn ship_script_composites_to_the_phase_0_byte_sequence() {
    let commands = parse_script(SHIP_SCRIPT).expect("ship.pxc must parse");
    let mut engine = Engine::new();
    engine.execute_all(commands).expect("ship.pxc must replay cleanly");
    let doc = engine.document().expect("ship.pxc must create a canvas");

    assert_eq!((doc.width(), doc.height()), (64, 64));
    assert_eq!(doc.layer_count(), 1, "ship.pxc is deliberately single-layer");

    let composite = doc.composite();
    assert_eq!(
        composite,
        doc.active_layer().pixels(),
        "a lone opaque layer must composite to its own buffer, byte for byte"
    );
    // The pinned value below was captured from a build whose CLI output
    // hashed to the documented Phase 0 PNG sha256.
    assert_eq!(fnv1a(&composite), SHIP_COMPOSITE_FNV1A);
}

/// FNV-1a over the composited buffer. Duplicated here (rather than reusing
/// `Document::content_hash`) on purpose: `content_hash` covers layer
/// metadata too, so it legitimately changes when layer defaults change,
/// whereas the *exported pixels* must not.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

const SHIP_COMPOSITE_FNV1A: u64 = 9_723_217_798_654_324_057;
