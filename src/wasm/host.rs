use anyhow::{Context, Result};
use wasmtime::{Memory, Store};

pub fn write_memory(store: &mut Store<()>, memory: &Memory, ptr: i32, bytes: &[u8]) -> Result<()> {
    memory
        .write(store, ptr as usize, bytes)
        .context("failed to write plugin memory")
}

pub fn read_memory(
    store: &mut Store<()>,
    memory: &Memory,
    ptr: i32,
    len: usize,
) -> Result<Vec<u8>> {
    let mut output = vec![0_u8; len];
    memory
        .read(store, ptr as usize, &mut output)
        .context("failed to read plugin memory")?;
    Ok(output)
}
