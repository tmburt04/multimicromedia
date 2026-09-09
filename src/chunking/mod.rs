use crate::error::{CompressionError, Result};
use crc32fast::Hasher;
use std::collections::HashSet;

pub const DEFAULT_CHUNK_SIZE: usize = crate::config::DEFAULT_CHUNK_SIZE_MB as usize * 1024 * 1024; // 64MB
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
        self.data.len().div_ceil(self.chunk_size) as u64
    }
}

impl<'a> Iterator for ChunkIterator<'a> {
    type Item = (ChunkHandle, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }

        let end = self
            .offset
            .saturating_add(self.chunk_size)
            .min(self.data.len());
        let chunk_data = &self.data[self.offset..end];
        let handle = ChunkHandle::new(self.chunk_id, self.offset as u64, chunk_data);

        self.offset = end;
        self.chunk_id += 1;

        Some((handle, chunk_data))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.data.len() - self.offset).div_ceil(self.chunk_size);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ChunkIterator<'_> {}
impl std::iter::FusedIterator for ChunkIterator<'_> {}

pub struct ChunkAssembler {
    chunks: Vec<(ChunkHandle, Vec<u8>)>,
    chunk_ids: HashSet<u64>,
    expected_total: Option<u64>,
}

impl ChunkAssembler {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            chunk_ids: HashSet::new(),
            expected_total: None,
        }
    }

    pub fn with_expected_total(mut self, total: u64) -> Self {
        self.expected_total = Some(total);
        self
    }

    pub fn add_chunk(&mut self, handle: ChunkHandle, data: Vec<u8>) -> Result<()> {
        if self.chunk_ids.contains(&handle.id)
            || self
                .expected_total
                .is_some_and(|total| handle.id >= total || self.chunks.len() as u64 >= total)
        {
            return Err(CompressionError::InvalidInput {
                reason: "duplicate or unexpected chunk ID".into(),
            });
        }
        if !handle.verify(&data) {
            return Err(CompressionError::ChunkCorrupted {
                chunk_id: handle.id,
                can_retry: true,
            });
        }

        if handle.offset.checked_add(handle.size).is_none() {
            return Err(CompressionError::InvalidInput {
                reason: "chunk byte range overflows".into(),
            });
        }
        self.chunks
            .try_reserve(1)
            .map_err(|_| CompressionError::MemoryLimitExceeded)?;
        self.chunk_ids
            .try_reserve(1)
            .map_err(|_| CompressionError::MemoryLimitExceeded)?;

        self.chunk_ids.insert(handle.id);
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
        if self.expected_total.is_some() && !self.is_complete() {
            return Err(CompressionError::PartialResult {
                completed_chunks: self.chunks.len() as u64,
                total_chunks: self.expected_total.unwrap_or(0),
                output: None,
            });
        }
        self.chunks
            .sort_unstable_by_key(|(h, _)| (h.offset, h.size));

        // Reject gaps, overlaps, and overflow before allocating the output.
        let mut expected_offset = 0u64;
        for (handle, _) in &self.chunks {
            if handle.offset != expected_offset {
                return Err(CompressionError::ChunkCorrupted {
                    chunk_id: handle.id,
                    can_retry: false,
                });
            }
            expected_offset = expected_offset
                .checked_add(handle.size)
                .ok_or(CompressionError::MemoryLimitExceeded)?;
        }
        let total_size =
            usize::try_from(expected_offset).map_err(|_| CompressionError::MemoryLimitExceeded)?;
        drop(self.chunk_ids);

        // Reuse the first allocation, including the common single-chunk case.
        let mut chunks = self.chunks.into_iter();
        let mut result = chunks.next().map_or_else(Vec::new, |(_, data)| data);
        result
            .try_reserve_exact(total_size - result.len())
            .map_err(|_| CompressionError::MemoryLimitExceeded)?;
        for (_, data) in chunks {
            result.extend_from_slice(&data);
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
    let chunk_bytes =
        ((chunk_size_mb as u64) * 1024 * 1024).clamp(MIN_CHUNK_SIZE as u64, MAX_CHUNK_SIZE as u64);
    size > chunk_bytes
}

pub fn calculate_optimal_chunk_size(_file_size: u64, max_memory_mb: u32) -> usize {
    let max_memory = (max_memory_mb as u64) * 1024 * 1024;

    // Aim for one quarter of available memory, subject to the 1-256 MiB chunk range.
    let max_chunk = (max_memory / 4).min(MAX_CHUNK_SIZE as u64) as usize;

    // But also don't make chunks too small
    max_chunk.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE)
}

pub struct StreamingProcessor<F> {
    processor: F,
    output: Vec<u8>,
    processed_chunks: u64,
    total_chunks: u64,
    input_size: u64,
    processed_bytes: u64,
    chunk_size: usize,
}

impl<F> StreamingProcessor<F>
where
    F: FnMut(&[u8]) -> Result<Vec<u8>>,
{
    pub fn new(input_size: u64, chunk_size: usize, processor: F) -> Self {
        let clamped = chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        let total_chunks = input_size.div_ceil(clamped as u64);

        Self {
            processor,
            output: Vec::new(),
            processed_chunks: 0,
            total_chunks,
            input_size,
            processed_bytes: 0,
            chunk_size: clamped,
        }
    }

    pub fn process_chunk(&mut self, data: &[u8]) -> Result<()> {
        let remaining = self.input_size - self.processed_bytes;
        let expected_size = remaining.min(self.chunk_size as u64) as usize;
        if expected_size == 0 || data.len() != expected_size {
            return Err(CompressionError::InvalidInput {
                reason: "chunk size does not match the remaining input".into(),
            });
        }
        let processed = (self.processor)(data)?;
        // Mixing raw and encoded chunks would corrupt a stream, so expansion
        // must be handled by the caller at the whole-file boundary.
        if processed.len() > data.len() {
            return Err(CompressionError::InvalidInput {
                reason: "streaming processor expanded a chunk".into(),
            });
        }
        self.output
            .try_reserve(processed.len())
            .map_err(|_| CompressionError::MemoryLimitExceeded)?;
        self.output.extend_from_slice(&processed);
        self.processed_chunks += 1;
        self.processed_bytes += data.len() as u64;
        Ok(())
    }

    pub fn progress(&self) -> (u64, u64) {
        (self.processed_chunks, self.total_chunks)
    }

    pub fn finalize(self) -> Vec<u8> {
        self.output
    }

    pub fn try_finalize(self) -> Result<Vec<u8>> {
        if self.processed_bytes != self.input_size {
            return Err(CompressionError::PartialResult {
                completed_chunks: self.processed_chunks,
                total_chunks: self.total_chunks,
                output: None,
            });
        }
        Ok(self.output)
    }
}
