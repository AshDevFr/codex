use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Compute SHA-256 hash of a file
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
}
