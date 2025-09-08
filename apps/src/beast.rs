//! BEAST mode protocol parser
//! 
//! BEAST mode is a binary format used by Mode S Beast hardware and dump1090-fa.
//! Frame format: <esc> <type> <6-byte timestamp> <signal> <message-data>
//! 
//! Escape character: 0x1A
//! Frame types:
//! - '1': Mode-AC (2 bytes)
//! - '2': Mode S short (7 bytes)
//! - '3': Mode S long (14 bytes)

use std::io::{self, Read};

/// BEAST mode escape character
const BEAST_SYNC: u8 = 0x1A;

/// BEAST frame types
#[derive(Debug, Clone, PartialEq)]
pub enum BeastFrameType {
    ModeAC = 0x31,      // '1'
    ModeSShort = 0x32,  // '2'
    ModeSLong = 0x33,   // '3'
}

impl TryFrom<u8> for BeastFrameType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x31 => Ok(BeastFrameType::ModeAC),
            0x32 => Ok(BeastFrameType::ModeSShort),
            0x33 => Ok(BeastFrameType::ModeSLong),
            _ => Err(format!("Unknown BEAST frame type: 0x{:02X}", value)),
        }
    }
}

/// Parsed BEAST frame
#[derive(Debug, Clone)]
pub struct BeastFrame {
    pub frame_type: BeastFrameType,
    pub timestamp_12mhz: u64,  // 48-bit timestamp (12MHz clock)
    pub signal_level: u8,
    pub message_data: Vec<u8>,
}

impl BeastFrame {
    /// Get the raw ADS-B message bytes (for use with Frame::from_bytes)
    pub fn message_bytes(&self) -> &[u8] {
        &self.message_data
    }

    /// Get timestamp in milliseconds (approximate)
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_12mhz / 12000  // Convert 12MHz ticks to milliseconds
    }
}

/// BEAST frame parser
pub struct BeastParser {
    buffer: Vec<u8>,
    sync_found: bool,
}

impl Default for BeastParser {
    fn default() -> Self {
        Self::new()
    }
}

impl BeastParser {
    /// Create a new BEAST parser
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(1024),
            sync_found: false,
        }
    }

    /// Parse data from reader and yield complete BEAST frames
    pub fn parse_frames<R: Read>(&mut self, reader: &mut R) -> io::Result<Vec<BeastFrame>> {
        let mut temp_buffer = [0u8; 1024];
        let bytes_read = match reader.read(&mut temp_buffer) {
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No data available right now, not a fatal error
                return Ok(Vec::new());
            }
            Err(e) => return Err(e), // Other I/O error
        };

        if bytes_read == 0 { // This indicates the stream was closed (EOF)
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "Stream closed"));
        }

        self.buffer.extend_from_slice(&temp_buffer[..bytes_read]);
        
        let mut frames = Vec::new();
        let mut pos = 0;

        while pos < self.buffer.len() {
            if !self.sync_found {
                // Look for sync byte
                if let Some(sync_pos) = self.buffer[pos..].iter().position(|&b| b == BEAST_SYNC) {
                    pos += sync_pos;
                    self.sync_found = true;
                    pos += 1; // Move past sync byte
                } else {
                    // No sync found, discard all data
                    self.buffer.clear();
                    break;
                }
            }

            if self.sync_found && pos < self.buffer.len() {
                // Try to parse frame
                match self.try_parse_frame_at(pos) {
                    Ok(Some((frame, frame_len))) => {
                        frames.push(frame);
                        pos += frame_len;
                        self.sync_found = false; // Look for next sync
                    }
                    Ok(None) => {
                        // Incomplete frame, need more data
                        break;
                    }
                    Err(_) => {
                        // Invalid frame, skip this byte and look for next sync
                        self.sync_found = false;
                        pos += 1;
                    }
                }
            }
        }

        // Remove processed data from buffer
        if pos > 0 && pos <= self.buffer.len() {
            self.buffer.drain(0..pos);
        }

        Ok(frames)
    }

    /// Try to parse a frame at the given position
    fn try_parse_frame_at(&self, pos: usize) -> Result<Option<(BeastFrame, usize)>, String> {
        if pos >= self.buffer.len() {
            return Ok(None);
        }

        let frame_type = BeastFrameType::try_from(self.buffer[pos])?;
        
        // Calculate expected frame size
        let message_len = match frame_type {
            BeastFrameType::ModeAC => 2,
            BeastFrameType::ModeSShort => 7,
            BeastFrameType::ModeSLong => 14,
        };

        let total_frame_len = 1 + 6 + 1 + message_len; // type + timestamp + signal + message
        
        // Check if we have enough data for complete frame
        if pos + total_frame_len > self.buffer.len() {
            return Ok(None); // Need more data
        }

        // Parse timestamp (6 bytes, big-endian, 48-bit)
        let mut timestamp = 0u64;
        for i in 0..6 {
            timestamp = (timestamp << 8) | (self.buffer[pos + 1 + i] as u64);
        }

        // Parse signal level
        let signal_level = self.buffer[pos + 7];

        // Extract message data
        let message_start = pos + 8;
        let message_data = self.buffer[message_start..message_start + message_len].to_vec();

        // Handle escape sequences in message data
        let unescaped_data = self.unescape_data(&message_data);

        let frame = BeastFrame {
            frame_type,
            timestamp_12mhz: timestamp,
            signal_level,
            message_data: unescaped_data,
        };

        Ok(Some((frame, total_frame_len)))
    }

    /// Unescape BEAST data (handle escaped 0x1A bytes)
    fn unescape_data(&self, data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut i = 0;
        
        while i < data.len() {
            if data[i] == BEAST_SYNC && i + 1 < data.len() && data[i + 1] == BEAST_SYNC {
                // Escaped sync byte
                result.push(BEAST_SYNC);
                i += 2;
            } else {
                result.push(data[i]);
                i += 1;
            }
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beast_frame_type_conversion() {
        assert_eq!(BeastFrameType::try_from(0x31).unwrap(), BeastFrameType::ModeAC);
        assert_eq!(BeastFrameType::try_from(0x32).unwrap(), BeastFrameType::ModeSShort);
        assert_eq!(BeastFrameType::try_from(0x33).unwrap(), BeastFrameType::ModeSLong);
        assert!(BeastFrameType::try_from(0x34).is_err());
    }

    #[test]
    fn test_beast_parser_creation() {
        let parser = BeastParser::new();
        assert!(!parser.sync_found);
        assert!(parser.buffer.is_empty());
    }

    #[test]
    fn test_escape_handling() {
        let parser = BeastParser::new();
        let data = vec![0x1A, 0x1A, 0x42]; // Escaped 0x1A followed by 0x42
        let result = parser.unescape_data(&data);
        assert_eq!(result, vec![0x1A, 0x42]);
    }

    #[test]
    fn test_timestamp_conversion() {
        let frame = BeastFrame {
            frame_type: BeastFrameType::ModeSLong,
            timestamp_12mhz: 12_000_000, // 1 second at 12MHz
            signal_level: 100,
            message_data: vec![0; 14],
        };
        assert_eq!(frame.timestamp_ms(), 1000); // Should be 1000ms
    }
}