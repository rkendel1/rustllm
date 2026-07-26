use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Envelope {
    hook: String,
    #[allow(dead_code)]
    config: serde_json::Value,
    payload: serde_json::Value,
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: i32) -> i32 {
    let mut buf = Vec::<u8>::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn dealloc(ptr: i32, len: i32) {
    unsafe {
        let _ = Vec::<u8>::from_raw_parts(ptr as *mut u8, 0, len as usize);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn on_request(ptr: i32, len: i32) -> i64 {
    run(ptr, len)
}

#[unsafe(no_mangle)]
pub extern "C" fn on_response(ptr: i32, len: i32) -> i64 {
    run(ptr, len)
}

#[unsafe(no_mangle)]
pub extern "C" fn on_stream_chunk(ptr: i32, len: i32) -> i64 {
    run(ptr, len)
}

#[unsafe(no_mangle)]
pub extern "C" fn on_auth(ptr: i32, len: i32) -> i64 {
    run(ptr, len)
}

fn run(ptr: i32, len: i32) -> i64 {
    let input = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    let envelope: Envelope = serde_json::from_slice(input).unwrap_or_else(|_| Envelope {
        hook: "unknown".to_string(),
        config: json!({}),
        payload: json!({}),
    });

    let payload_string = envelope.payload.to_string().to_lowercase();
    let blocked = payload_string.contains("password") || payload_string.contains("credit card");

    let output = if envelope.hook == "on_request" && blocked {
        json!({"allow": false, "reject_reason": "blocked by keyword guardrail"})
    } else {
        json!({"allow": true, "body": envelope.payload})
    };

    let bytes = serde_json::to_vec(&output).unwrap_or_else(|_| b"{\"allow\":true}".to_vec());
    let len = bytes.len() as i32;
    let out_ptr = alloc(len);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr as *mut u8, len as usize);
    }

    ((out_ptr as i64) << 32) | (len as u32 as i64)
}
