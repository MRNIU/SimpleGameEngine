// Copyright The SimpleGameEngine Contributors

use sha2::{Digest, Sha256};

use super::hash_frame;

#[test]
fn adjacent_frames_prevent_split_collisions() {
    let digest = |frames: &[&[u8]]| {
        let mut hasher = Sha256::new();
        for frame in frames {
            hash_frame(&mut hasher, frame);
        }
        hasher.finalize()
    };

    assert_ne!(digest(&[b"ab", b"c"]), digest(&[b"a", b"bc"]));
}
