#[derive(Debug, Clone, PartialEq)]
pub struct DraftMemory;

pub trait DraftMemoryBlock {
    const BYTE_SIZE: usize;
    fn as_bytes(&self) -> &[u8; BYTE_SIZE];
    fn from_bytes(bytes: &[u8]);
}
