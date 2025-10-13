use std::io;

use super::loader::MapLoader;

/// MapLoader implementation for http:// and https:// URIs
pub struct HttpMapLoader {
    agent: ureq::Agent,
}

impl HttpMapLoader {
    pub fn new() -> Self {
        Self {
            agent: ureq::Agent::new(),
        }
    }

    /// Construct the full URL for a resource within the map directory
    fn construct_url(&self, base_uri: &str, resource: &str) -> String {
        let base = base_uri.trim_end_matches('/');
        format!("{}/{}", base, resource)
    }

    /// Perform HTTP GET and return the response as bytes
    fn fetch(&self, url: &str) -> io::Result<Vec<u8>> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(bytes)
    }
}

impl Default for HttpMapLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl MapLoader for HttpMapLoader {
    fn scheme(&self) -> &str {
        "http"
    }

    fn load_manifest(&self, uri: &str) -> io::Result<String> {
        let url = self.construct_url(uri, "manifest.json");
        let bytes = self.fetch(&url)?;
        String::from_utf8(bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    fn load_map_binary(&self, uri: &str) -> io::Result<Vec<u8>> {
        let url = self.construct_url(uri, "map.bin");
        self.fetch(&url)
    }
}

/// MapLoader implementation specifically for https:// URIs
/// This is a separate struct to allow registration of both http and https schemes
pub struct HttpsMapLoader {
    inner: HttpMapLoader,
}

impl HttpsMapLoader {
    pub fn new() -> Self {
        Self {
            inner: HttpMapLoader::new(),
        }
    }
}

impl Default for HttpsMapLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl MapLoader for HttpsMapLoader {
    fn scheme(&self) -> &str {
        "https"
    }

    fn load_manifest(&self, uri: &str) -> io::Result<String> {
        self.inner.load_manifest(uri)
    }

    fn load_map_binary(&self, uri: &str) -> io::Result<Vec<u8>> {
        self.inner.load_map_binary(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_url() {
        let loader = HttpMapLoader::new();

        let url = loader.construct_url("http://example.com/maps/africa", "manifest.json");
        assert_eq!(url, "http://example.com/maps/africa/manifest.json");

        let url = loader.construct_url("http://example.com/maps/africa/", "map.bin");
        assert_eq!(url, "http://example.com/maps/africa/map.bin");
    }

    #[test]
    fn test_schemes() {
        let http_loader = HttpMapLoader::new();
        assert_eq!(http_loader.scheme(), "http");

        let https_loader = HttpsMapLoader::new();
        assert_eq!(https_loader.scheme(), "https");
    }
}
