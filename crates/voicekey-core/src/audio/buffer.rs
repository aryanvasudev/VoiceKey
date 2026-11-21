//! Circular ring buffer with pre-roll memory to capture initial audio consonants.

/// A circular pre-roll audio buffer that maintains the last N milliseconds of audio.
///
/// When the user activates dictation, speech often precedes the hotkey event
/// by 50-150ms. By maintaining a continuous circular buffer (typically 200ms)
/// and prepending it when recording starts, initial consonants are preserved.
#[derive(Debug, Clone)]
pub struct PreRollBuffer {
    capacity: usize,
    sample_rate: u32,
    buffer: Vec<f32>,
    write_pos: usize,
    count: usize,
}

impl PreRollBuffer {
    /// Create a new pre-roll buffer for the given sample rate and duration in milliseconds.
    ///
    /// For example: 16000 Hz with 200ms results in a capacity of 3,200 samples.
    pub fn new(sample_rate: u32, duration_ms: u32) -> Self {
        let capacity = ((sample_rate as f64 * duration_ms as f64) / 1000.0).round() as usize;
        let capacity = capacity.max(1);
        Self {
            capacity,
            sample_rate,
            buffer: vec![0.0; capacity],
            write_pos: 0,
            count: 0,
        }
    }

    /// Push a slice of mono audio samples into the circular buffer.
    pub fn push_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            if self.count < self.capacity {
                self.count += 1;
            }
        }
    }

    /// Extract the buffered pre-roll samples in chronological order.
    pub fn get_pre_roll(&self) -> Vec<f32> {
        if self.count == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(self.count);
        if self.count < self.capacity {
            // Buffer hasn't wrapped yet: samples are in 0..self.count
            result.extend_from_slice(&self.buffer[0..self.count]);
        } else {
            // Buffer has wrapped: oldest sample is at self.write_pos
            result.extend_from_slice(&self.buffer[self.write_pos..self.capacity]);
            result.extend_from_slice(&self.buffer[0..self.write_pos]);
        }
        result
    }

    /// Reset and clear the buffer contents.
    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.count = 0;
    }

    /// Total capacity in samples.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Current number of valid samples in buffer.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The configured sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_capacity() {
        let buf = PreRollBuffer::new(16000, 200);
        assert_eq!(buf.capacity(), 3200);
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_partial_fill() {
        let mut buf = PreRollBuffer::new(1000, 10); // 10 samples
        buf.push_samples(&[1.0, 2.0, 3.0]);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.get_pre_roll(), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_wrap_around_chronological_order() {
        let mut buf = PreRollBuffer::new(1000, 5); // capacity = 5 samples
        buf.push_samples(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(buf.get_pre_roll(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);

        // Push 2 more -> oldest 1.0, 2.0 should be evicted
        buf.push_samples(&[6.0, 7.0]);
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.get_pre_roll(), vec![3.0, 4.0, 5.0, 6.0, 7.0]);
    }

    #[test]
    fn test_clear() {
        let mut buf = PreRollBuffer::new(16000, 200);
        buf.push_samples(&[0.5; 100]);
        assert_eq!(buf.len(), 100);
        buf.clear();
        assert_eq!(buf.len(), 0);
        assert!(buf.get_pre_roll().is_empty());
    }
}
