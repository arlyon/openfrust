use std::io;

/// Trait for loading map data from different sources identified by URI schemes
pub trait MapLoader: Send + Sync {
    /// Returns the URI scheme this loader handles (e.g., "file", "http", "https")
    fn scheme(&self) -> &str;

    /// Load manifest.json from the given URI
    fn load_manifest(&self, uri: &str) -> io::Result<String>;

    /// Load map.bin from the given URI
    fn load_map_binary(&self, uri: &str) -> io::Result<Vec<u8>>;
}

/// Manages multiple MapLoader implementations and routes requests based on URI scheme
pub struct MapLoaderManager {
    loaders: Vec<Box<dyn MapLoader>>,
}

impl MapLoaderManager {
    /// Create a new empty MapLoaderManager
    pub fn new() -> Self {
        Self {
            loaders: Vec::new(),
        }
    }

    /// Register a new map loader
    pub fn register<T: MapLoader + 'static>(&mut self, loader: T) {
        self.loaders.push(Box::new(loader));
    }

    /// Find the appropriate loader for a given URI
    fn find_loader(&self, uri: &str) -> io::Result<&dyn MapLoader> {
        let scheme = uri
            .split("://")
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid URI format"))?;

        self.loaders
            .iter()
            .find(|loader| loader.scheme() == scheme)
            .map(|boxed| &**boxed)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("No loader found for scheme: {}", scheme),
                )
            })
    }

    /// Load manifest from URI
    pub fn load_manifest(&self, uri: &str) -> io::Result<String> {
        let loader = self.find_loader(uri)?;
        loader.load_manifest(uri)
    }

    /// Load map binary from URI
    pub fn load_map_binary(&self, uri: &str) -> io::Result<Vec<u8>> {
        let loader = self.find_loader(uri)?;
        loader.load_map_binary(uri)
    }
}

impl Default for MapLoaderManager {
    fn default() -> Self {
        Self::new()
    }
}
