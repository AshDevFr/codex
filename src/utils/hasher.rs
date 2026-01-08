use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Compute SHA-256 hash of entire file
pub fn hash_file<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute SHA-256 hash of first 1MB of a file (partial hash for fast change detection)
pub fn hash_file_partial<P: AsRef<Path>>(path: P) -> io::Result<String> {
    const HASH_READ_SIZE: usize = 1024 * 1024; // 1MB

    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_READ_SIZE];

    // Read up to HASH_READ_SIZE bytes, handling partial reads
    let bytes_read = match file.read(&mut buffer)? {
        0 => 0, // Empty file
        n => {
            // First read got n bytes, but we want to read up to HASH_READ_SIZE total
            let mut total_read = n;
            while total_read < HASH_READ_SIZE {
                match file.read(&mut buffer[total_read..]) {
                    Ok(0) => break, // EOF
                    Ok(n) => total_read += n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
            total_read
        }
    };

    hasher.update(&buffer[..bytes_read]);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_file_empty() {
        let temp_file = NamedTempFile::new().unwrap();
        let hash = hash_file(temp_file.path()).unwrap();

        // SHA-256 hash of empty file
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_hash_file_with_content() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Hello, World!").unwrap();
        temp_file.flush().unwrap();

        let hash = hash_file(temp_file.path()).unwrap();

        // SHA-256 hash of "Hello, World!"
        assert_eq!(
            hash,
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
        );
    }

    #[test]
    fn test_hash_file_large_content() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // Write 10KB of data (larger than the 8192 buffer)
        let data = vec![42u8; 10240];
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let hash = hash_file(temp_file.path()).unwrap();

        // Hash should be consistent
        assert_eq!(hash.len(), 64); // SHA-256 produces 64 hex characters
    }

    #[test]
    fn test_hash_file_nonexistent() {
        let result = hash_file("/nonexistent/path/to/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_file_deterministic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test content").unwrap();
        temp_file.flush().unwrap();

        let hash1 = hash_file(temp_file.path()).unwrap();
        let hash2 = hash_file(temp_file.path()).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_file_partial_empty() {
        let temp_file = NamedTempFile::new().unwrap();
        let hash = hash_file_partial(temp_file.path()).unwrap();

        // SHA-256 hash of empty file (same as full hash for empty file)
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_hash_file_partial_small() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"Hello, World!").unwrap();
        temp_file.flush().unwrap();

        let partial_hash = hash_file_partial(temp_file.path()).unwrap();
        let full_hash = hash_file(temp_file.path()).unwrap();

        // For files smaller than 1MB, partial and full hash should be identical
        assert_eq!(partial_hash, full_hash);
        assert_eq!(
            partial_hash,
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
        );
    }

    #[test]
    fn test_hash_file_partial_large() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // Write 2MB of data (larger than 1MB partial hash size)
        let data = vec![42u8; 2 * 1024 * 1024];
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let partial_hash = hash_file_partial(temp_file.path()).unwrap();
        let full_hash = hash_file(temp_file.path()).unwrap();

        // For files larger than 1MB, hashes should differ
        assert_ne!(partial_hash, full_hash);
        // Both should be valid SHA-256 hashes (64 hex chars)
        assert_eq!(partial_hash.len(), 64);
        assert_eq!(full_hash.len(), 64);
    }

    #[test]
    fn test_hash_file_partial_deterministic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let data = vec![123u8; 500_000];
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let hash1 = hash_file_partial(temp_file.path()).unwrap();
        let hash2 = hash_file_partial(temp_file.path()).unwrap();

        // Partial hash should be deterministic
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_file_partial_exactly_1mb() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // Write exactly 1MB
        let data = vec![55u8; 1024 * 1024];
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let partial_hash = hash_file_partial(temp_file.path()).unwrap();
        let full_hash = hash_file(temp_file.path()).unwrap();

        // For exactly 1MB file, both hashes should be identical
        assert_eq!(partial_hash, full_hash);
    }

    #[test]
    fn test_hash_file_partial_just_over_1mb() {
        let mut temp_file = NamedTempFile::new().unwrap();
        // Write 1MB + 1 byte
        let mut data = vec![77u8; 1024 * 1024];
        data.push(99);
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let partial_hash = hash_file_partial(temp_file.path()).unwrap();
        let full_hash = hash_file(temp_file.path()).unwrap();

        // Hashes should differ (partial only reads first 1MB)
        assert_ne!(partial_hash, full_hash);
    }
}
