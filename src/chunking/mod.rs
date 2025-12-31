use crate::error::{CompressionError, Result};
use crc32fast::Hasher;

pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024 * 1024; // 64MB
pub const MIN_CHUNK_SIZE: usize = 1024 * 1024; // 1MB
pub const MAX_CHUNK_SIZE: usize = 256 * 1024 * 1024; // 256MB

#[derive(Debug, Clone)]
pub struct ChunkHandle {
    pub id: u64,
    pub offset: u64,
    pub size: u64,
    pub checksum: u32,
}

impl ChunkHandle {
    pub fn new(id: u64, offset: u64, data: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(data);
        let checksum = hasher.finalize();

        Self {
            id,
            offset,
            size: data.len() as u64,
            checksum,
        }
    }

    pub fn verify(&self, data: &[u8]) -> bool {
        if data.len() as u64 != self.size {
            return false;
        }

        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize() == self.checksum
    }
}

#[derive(Debug)]
pub struct ChunkIterator<'a> {
    data: &'a [u8],
    chunk_size: usize,
    offset: usize,
    chunk_id: u64,
}

impl<'a> ChunkIterator<'a> {
    pub fn new(data: &'a [u8], chunk_size: usize) -> Self {
        let chunk_size = chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        Self {
            data,
            chunk_size,
            offset: 0,
            chunk_id: 0,
        }
    }

    pub fn total_chunks(&self) -> u64 {
        ((self.data.len() + self.chunk_size - 1) / self.chunk_size) as u64
    }
}

impl<'a> Iterator for ChunkIterator<'a> {
    type Item = (ChunkHandle, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }

        let end = (self.offset + self.chunk_size).min(self.data.len());
        let chunk_data = &self.data[self.offset..end];
        let handle = ChunkHandle::new(self.chunk_id, self.offset as u64, chunk_data);

        self.offset = end;
        self.chunk_id += 1;

        Some((handle, chunk_data))
    }
}

pub struct ChunkAssembler {
    chunks: Vec<(ChunkHandle, Vec<u8>)>,
    expected_total: Option<u64>,
}

impl ChunkAssembler {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            expected_total: None,
        }
    }

    pub fn with_expected_total(mut self, total: u64) -> Self {
        self.expected_total = Some(total);
        self.chunks.reserve(total as usize);
        self
    }

    pub fn add_chunk(&mut self, handle: ChunkHandle, data: Vec<u8>) -> Result<()> {
        if !handle.verify(&data) {
            return Err(CompressionError::ChunkCorrupted {
                chunk_id: handle.id,
                can_retry: true,
            });
        }

        self.chunks.push((handle, data));
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        match self.expected_total {
            Some(total) => self.chunks.len() as u64 == total,
            None => false,
        }
    }

    pub fn progress(&self) -> (u64, Option<u64>) {
        (self.chunks.len() as u64, self.expected_total)
    }

    pub fn assemble(mut self) -> Result<Vec<u8>> {
        // Sort by offset
        self.chunks.sort_by_key(|(h, _)| h.offset);

        // Calculate total size
        let total_size: u64 = self.chunks.iter().map(|(h, _)| h.size).sum();
        let mut result = Vec::with_capacity(total_size as usize);

        // Verify contiguity and assemble
        let mut expected_offset = 0u64;
        for (handle, data) in self.chunks {
            if handle.offset != expected_offset {
                return Err(CompressionError::ChunkCorrupted {
                    chunk_id: handle.id,
                    can_retry: false,
                });
            }
            result.extend_from_slice(&data);
            expected_offset += handle.size;
        }

        Ok(result)
    }
}

impl Default for ChunkAssembler {
    fn default() -> Self {
        Self::new()
    }
}

pub fn should_chunk(size: u64, chunk_size_mb: u32) -> bool {
    let chunk_bytes = (chunk_size_mb as u64) * 1024 * 1024;
    size > chunk_bytes
}

pub fn calculate_optimal_chunk_size(_file_size: u64, max_memory_mb: u32) -> usize {
    let max_memory = (max_memory_mb as u64) * 1024 * 1024;
    
    // Use at most 1/4 of available memory per chunk to allow for processing overhead
    let max_chunk = (max_memory / 4) as usize;
    
    // But also don't make chunks too small
    max_chunk.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE)
}

pub struct StreamingProcessor<F> {
    processor: F,
    output: Vec<u8>,
    processed_chunks: u64,
    total_chunks: u64,
}

impl<F> StreamingProcessor<F>
where
    F: FnMut(&[u8]) -> Result<Vec<u8>>,
{
    pub fn new(input_size: u64, chunk_size: usize, processor: F) -> Self {
        let clamped = chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        let total_chunks = ((input_size as usize + clamped - 1) / clamped) as u64;

        Self {
            processor,
            output: Vec::new(),
            processed_chunks: 0,
            total_chunks,
        }
    }

    pub fn process_chunk(&mut self, data: &[u8]) -> Result<()> {
        let processed = (self.processor)(data)?;
        self.output.extend_from_slice(&processed);
        self.processed_chunks += 1;
        Ok(())
    }

    pub fn progress(&self) -> (u64, u64) {
        (self.processed_chunks, self.total_chunks)
    }

    pub fn finalize(self) -> Vec<u8> {
        self.output
    }
}
