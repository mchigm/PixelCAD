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
