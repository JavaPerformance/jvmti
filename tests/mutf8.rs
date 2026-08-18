use jvmti_bindings::mutf8::{self, Mutf8ErrorKind};

#[test]
fn java_modified_utf8_round_trips_rust_strings() {
    for value in [
        "",
        "ASCII",
        "embedded\0nul",
        "Gruesse aus Zuerich",
        "日本語",
        "supplementary: \u{1f680}",
    ] {
        let encoded = mutf8::encode(value);
        assert!(!encoded.contains(&0));
        assert_eq!(mutf8::decode(&encoded).unwrap(), value);
        assert_eq!(
            mutf8::decode_cstr(mutf8::encode_cstring(value).as_c_str()).unwrap(),
            value
        );
    }
}

#[test]
fn modified_utf8_uses_java_null_and_surrogate_encodings() {
    assert_eq!(mutf8::encode("\0"), [0xc0, 0x80]);
    assert_eq!(
        mutf8::encode("\u{1f680}"),
        [0xed, 0xa0, 0xbd, 0xed, 0xba, 0x80]
    );
    assert_eq!(
        mutf8::decode_utf16(&[0xed, 0xa0, 0xbd, 0xed, 0xba, 0x80]).unwrap(),
        [0xd83d, 0xde80]
    );
    assert_eq!(
        mutf8::encode_utf16(&[0xd800, b'A' as u16, 0xdc00]),
        [0xed, 0xa0, 0x80, b'A', 0xed, 0xb0, 0x80]
    );
}

#[test]
fn cow_decode_borrows_compatible_text_and_owns_java_special_forms() {
    assert!(matches!(
        mutf8::decode_cow("Grüße".as_bytes()).unwrap(),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(matches!(
        mutf8::decode_cow(&mutf8::encode("\0\u{1f680}")).unwrap(),
        std::borrow::Cow::Owned(_)
    ));
}

#[test]
fn exact_utf16_decode_preserves_unpaired_java_surrogates() {
    let high_surrogate = [0xed, 0xa0, 0x80];
    assert_eq!(mutf8::decode_utf16(&high_surrogate).unwrap(), [0xd800]);
    let error = mutf8::decode(&high_surrogate).unwrap_err();
    assert_eq!(error.kind(), Mutf8ErrorKind::UnpairedSurrogate);
    assert_eq!(error.offset(), 0);
    assert_eq!(mutf8::decode_lossy(&high_surrogate), "\u{fffd}");

    let after_prefix = [b'A', 0xed, 0xa0, 0x80];
    let error = mutf8::decode(&after_prefix).unwrap_err();
    assert_eq!(error.kind(), Mutf8ErrorKind::UnpairedSurrogate);
    assert_eq!(error.offset(), 1);

    let valid_pair_then_unpaired_low = [0xed, 0xa0, 0xbd, 0xed, 0xba, 0x80, 0xed, 0xb0, 0x80];
    let error = mutf8::decode(&valid_pair_then_unpaired_low).unwrap_err();
    assert_eq!(error.kind(), Mutf8ErrorKind::UnpairedSurrogate);
    assert_eq!(error.offset(), 6);
}

#[test]
fn malformed_modified_utf8_is_rejected_with_a_location() {
    let cases = [
        (&[0][..], Mutf8ErrorKind::EmbeddedNul),
        (&[0xc2][..], Mutf8ErrorKind::UnexpectedEnd),
        (&[0xc2, b'A'][..], Mutf8ErrorKind::InvalidContinuationByte),
        (&[0xc1, 0x81][..], Mutf8ErrorKind::OverlongEncoding),
        (
            &[0xf0, 0x90, 0x80, 0x80][..],
            Mutf8ErrorKind::InvalidLeadingByte,
        ),
    ];
    for (bytes, expected) in cases {
        assert_eq!(mutf8::validate(bytes).unwrap_err().kind(), expected);
        assert_eq!(mutf8::decode(bytes).unwrap_err().kind(), expected);
    }
}

#[test]
fn validation_accepts_exact_unpaired_java_surrogates_without_allocating() {
    assert!(mutf8::validate(&mutf8::encode_utf16(&[0xd800, 0xdc00, 0xdfff])).is_ok());
}

#[test]
fn c_string_decode_excludes_only_the_terminator() {
    let value = c"abc";
    assert_eq!(mutf8::decode_cstr(value).unwrap(), "abc");
}
