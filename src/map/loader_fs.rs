use std::fs;
use std::io;
use std::path::PathBuf;

use super::loader::MapLoader;

/// MapLoader implementation for file:// URIs
pub struct FileSystemMapLoader;

impl FileSystemMapLoader {
    pub fn new() -> Self {
        Self
    }

    /// Parse file:// URI to extract the path
    fn parse_uri(&self, uri: &str) -> io::Result<PathBuf> {
        if !uri.starts_with("file://") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "URI must start with file://",
            ));
        }

        let path_str = &uri[7..]; // Remove "file://"
        Ok(PathBuf::from(path_str))
    }
}

impl Default for FileSystemMapLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl MapLoader for FileSystemMapLoader {
    fn scheme(&self) -> &str {
        "file"
    }

    fn load_manifest(&self, uri: &str) -> io::Result<String> {
        let base_path = self.parse_uri(uri)?;
        let manifest_path = base_path.join("manifest.json");
        fs::read_to_string(&manifest_path)
    }

    fn load_map_binary(&self, uri: &str) -> io::Result<Vec<u8>> {
        let base_path = self.parse_uri(uri)?;
        let map_path = base_path.join("map.bin");
        fs::read(&map_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uri() {
        let loader = FileSystemMapLoader::new();

        let path = loader.parse_uri("file:///home/user/maps/africa").unwrap();
        assert_eq!(path, PathBuf::from("/home/user/maps/africa"));

        let path = loader.parse_uri("file://assets/maps/world").unwrap();
        assert_eq!(path, PathBuf::from("assets/maps/world"));

        let result = loader.parse_uri("http://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_scheme() {
        let loader = FileSystemMapLoader::new();
        assert_eq!(loader.scheme(), "file");
    }
}
