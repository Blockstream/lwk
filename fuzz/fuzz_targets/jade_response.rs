#![no_main]

use std::collections::BTreeMap;

use libfuzzer_sys::fuzz_target;
use lwk_jade::{fuzz_try_parse_response, protocol::Response, FuzzParseStep};
use serde_cbor::Value;

const MAX_RESPONSE_BYTES: usize = 4096;
const MAX_STRUCTURED_PAYLOAD_BYTES: usize = 512;
const EXPECTED_ID: &str = "1";

fn parse_buffer(mut buffer: &[u8]) -> FuzzParseStep {
    let mut remaining_steps = buffer.len() + 1;
    loop {
        assert!(remaining_steps > 0, "skipping must always make progress");
        remaining_steps -= 1;

        match fuzz_try_parse_response(buffer, EXPECTED_ID) {
            FuzzParseStep::Skip { consumed } => {
                assert!(consumed > 0);
                assert!(consumed <= buffer.len());
                buffer = &buffer[consumed..];
            }
            step => return step,
        }
    }
}

fn response(id: &str, result: &[u8]) -> Vec<u8> {
    serde_cbor::to_vec(&Response {
        id: id.to_string(),
        result: Some(Value::Bytes(result.to_vec())),
        error: None,
        seqnum: None,
        seqlen: None,
    })
    .expect("response serialization succeeds")
}

fn log_message(data: &[u8]) -> Vec<u8> {
    let value = Value::Map(BTreeMap::from([(
        Value::Text("log".to_string()),
        Value::Bytes(data.to_vec()),
    )]));
    serde_cbor::to_vec(&value).expect("log serialization succeeds")
}

fn unmatched_error(data: &[u8]) -> Vec<u8> {
    let error = Value::Map(BTreeMap::from([
        (Value::Text("code".to_string()), Value::Integer(-32600)),
        (
            Value::Text("message".to_string()),
            Value::Text(String::from_utf8_lossy(data).into_owned()),
        ),
        (Value::Text("data".to_string()), Value::Bytes(data.to_vec())),
    ]));
    let value = Value::Map(BTreeMap::from([
        (Value::Text("id".to_string()), Value::Text("00".to_string())),
        (Value::Text("error".to_string()), error),
    ]));
    serde_cbor::to_vec(&value).expect("error serialization succeeds")
}

fn exercise_prefixes(buffer: &[u8]) {
    let split_points = [
        0,
        1.min(buffer.len()),
        buffer.len() / 4,
        buffer.len() / 2,
        buffer.len().saturating_sub(1),
        buffer.len(),
    ];
    for end in split_points {
        let _ = parse_buffer(&buffer[..end]);
    }
}

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_RESPONSE_BYTES)];
    let structured_data = &data[..data.len().min(MAX_STRUCTURED_PAYLOAD_BYTES)];

    exercise_prefixes(data);

    let expected = response(EXPECTED_ID, structured_data);
    assert_eq!(parse_buffer(&expected), FuzzParseStep::Mine);
    exercise_prefixes(&expected);

    let mut stale_then_expected = response("2", structured_data);
    stale_then_expected.extend_from_slice(&expected);
    assert_eq!(parse_buffer(&stale_then_expected), FuzzParseStep::Mine);
    exercise_prefixes(&stale_then_expected);

    let mut log_then_expected = log_message(structured_data);
    log_then_expected.extend_from_slice(&expected);
    assert_eq!(parse_buffer(&log_then_expected), FuzzParseStep::Mine);
    exercise_prefixes(&log_then_expected);

    assert_eq!(
        parse_buffer(&unmatched_error(structured_data)),
        FuzzParseStep::Mine
    );

    let mut expected_then_trailing = expected;
    let remaining = MAX_RESPONSE_BYTES.saturating_sub(expected_then_trailing.len());
    expected_then_trailing.extend_from_slice(&data[..data.len().min(remaining)]);
    assert_eq!(parse_buffer(&expected_then_trailing), FuzzParseStep::Mine);
});
