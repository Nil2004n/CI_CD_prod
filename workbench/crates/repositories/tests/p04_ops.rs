use repositories::git::ops::{GitRefusal, validate_target_filters};

#[test]
fn target_filters_accept_opaque_paths_in_exact_order() {
    let paths = b"space name\0line\nname\0\xff\x01\0";
    let attrs = b"space name\0filter\0unset\0line\nname\0filter\0unset\0\xff\x01\0filter\0unset\0";
    assert_eq!(validate_target_filters(paths, attrs), Ok(()));
}

#[test]
fn target_filters_refuse_incomplete_or_unsafe_results() {
    let bad: &[(&[u8], &[u8])] = &[
        (b"", b""),
        (b"a", b"a\0filter\0unset\0"),
        (b"a\0", b"a\0filter\0unset"),
        (b"a\0", b"a\0filter\0"),
        (b"a\0", b"a\0filter\0set\0"),
        (b"a\0", b"b\0filter\0unset\0"),
        (b"a\0", b"a\0filter\0unset\0extra\0"),
        (b"a\0a\0", b"a\0filter\0unset\0a\0filter\0unset\0"),
        (b"\0", b"\0filter\0unset\0"),
    ];
    for (paths, attrs) in bad {
        assert_eq!(
            validate_target_filters(paths, attrs),
            Err(GitRefusal::UnsafeAttributes)
        );
    }
    assert_eq!(
        validate_target_filters(&vec![b'a'; 4097], b""),
        Err(GitRefusal::UnsafeAttributes)
    );
}
