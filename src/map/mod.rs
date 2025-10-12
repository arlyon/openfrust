use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use bevy::prelude::Resource;
use bitfield::bitfield;
use serde::Deserialize;

bitfield! {
    /// Represents immutable terrain data for a single map tile using a compact bitfield.
    /// Matches the TypeScript implementation's bit layout.
    #[derive(Clone, Copy)]
    pub struct MapTile(u8);
    impl Debug;
    pub is_land, _: 7;           // Bit 7: Is this a land tile?
    pub is_shoreline, _: 6;      // Bit 6: Is this on a shoreline?
    pub is_ocean, _: 5;          // Bit 5: Is this ocean water?
    pub u8, magnitude, _: 4, 0;  // Bits 0-4: Terrain magnitude (0-31)
}

impl MapTile {
    /// Create a new MapTile from raw byte data
    pub fn from_byte(byte: u8) -> Self {
        MapTile(byte)
    }

    /// Get the raw byte value
    pub fn as_byte(&self) -> u8 {
        self.0
    }

    /// Check if this tile is water (not land)
    pub fn is_water(&self) -> bool {
        !self.is_land()
    }

    /// Check if this is a lake (water but not ocean)
    pub fn is_lake(&self) -> bool {
        !self.is_land() && !self.is_ocean()
    }

    /// Check if this is a shore (land and shoreline)
    pub fn is_shore(&self) -> bool {
        self.is_land() && self.is_shoreline()
    }

    /// Get movement cost for this tile
    pub fn cost(&self) -> u8 {
        if self.magnitude() < 10 { 2 } else { 1 }
    }

    /// Get terrain type based on tile properties
    pub fn terrain_type(&self) -> TerrainType {
        if self.is_land() {
            let magnitude = self.magnitude();
            if magnitude < 10 {
                TerrainType::Plains
            } else if magnitude < 20 {
                TerrainType::Highland
            } else {
                TerrainType::Mountain
            }
        } else if self.is_ocean() {
            TerrainType::Ocean
        } else {
            TerrainType::Lake
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Ocean,
    Lake,
    Plains,
    Highland,
    Mountain,
}

/// A tile reference is simply an index into the map array
pub type TileRef = usize;

#[derive(Debug, Clone, Deserialize)]
pub struct Nation {
    pub name: String,
    pub flag: String,
    pub coordinates: (u32, u32),
    pub strength: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct MapInfo {
    width: u32,
    height: u32,
    num_land_tiles: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct MapManifest {
    name: String,
    map: MapInfo,
    nations: Vec<Nation>,
}

/// Represents a game map with immutable terrain data
#[derive(Resource)]
pub struct GameMap {
    name: String,
    width: u32,
    height: u32,
    num_land_tiles: u32,
    terrain: Vec<MapTile>,
    nations: Vec<Nation>,

    // Lookup tables (LUTs) for fast coordinate conversion
    ref_to_x: Vec<u32>,
    ref_to_y: Vec<u32>,
    y_to_ref: Vec<usize>,
}

impl GameMap {
    /// Load a map by name from the assets/maps directory
    pub fn load(name: &str) -> io::Result<Self> {
        let base_path = PathBuf::from("assets/maps").join(name);
        Self::load_from_path(&base_path)
    }

    /// Load a map from a specific path
    pub fn load_from_path(path: &Path) -> io::Result<Self> {
        // Read and parse manifest
        let manifest_path = path.join("manifest.json");
        let manifest_data = fs::read_to_string(&manifest_path)?;
        let manifest: MapManifest = serde_json::from_str(&manifest_data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Read binary terrain data
        let map_path = path.join("map.bin");
        let terrain_bytes = fs::read(&map_path)?;

        // Validate terrain data length
        let expected_size = (manifest.map.width * manifest.map.height) as usize;
        if terrain_bytes.len() != expected_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Terrain data length {} doesn't match dimensions {}x{} (expected {})",
                    terrain_bytes.len(),
                    manifest.map.width,
                    manifest.map.height,
                    expected_size
                ),
            ));
        }

        // Convert bytes to MapTile bitfields
        let terrain: Vec<MapTile> = terrain_bytes.into_iter().map(MapTile::from_byte).collect();

        // Precompute lookup tables
        let width = manifest.map.width;
        let height = manifest.map.height;
        let total_tiles = (width * height) as usize;

        let mut ref_to_x = Vec::with_capacity(total_tiles);
        let mut ref_to_y = Vec::with_capacity(total_tiles);
        let mut y_to_ref = Vec::with_capacity(height as usize);

        let mut tile_ref = 0;
        for y in 0..height {
            y_to_ref.push(tile_ref);
            for x in 0..width {
                ref_to_x.push(x);
                ref_to_y.push(y);
                tile_ref += 1;
            }
        }

        Ok(Self {
            name: manifest.name,
            width,
            height,
            num_land_tiles: manifest.map.num_land_tiles,
            terrain,
            nations: manifest.nations,
            ref_to_x,
            ref_to_y,
            y_to_ref,
        })
    }

    /// Scan the assets/maps directory and return all available map names
    pub fn scan_available_maps() -> io::Result<Vec<String>> {
        let maps_dir = Path::new("assets/maps");
        let mut map_names = Vec::new();

        if !maps_dir.exists() {
            return Ok(map_names);
        }

        for entry in fs::read_dir(maps_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Check if it's a directory with a manifest.json
            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        map_names.push(name.to_string());
                    }
                }
            }
        }

        map_names.sort();
        Ok(map_names)
    }

    // Getters
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn num_land_tiles(&self) -> u32 {
        self.num_land_tiles
    }

    pub fn nations(&self) -> &[Nation] {
        &self.nations
    }

    /// Get the terrain data as a slice of MapTiles
    pub fn terrain(&self) -> &[MapTile] {
        &self.terrain
    }

    // Coordinate conversion
    pub fn tile_ref(&self, x: u32, y: u32) -> Option<TileRef> {
        if self.is_valid_coord(x, y) {
            Some(self.y_to_ref[y as usize] + x as usize)
        } else {
            None
        }
    }

    pub fn is_valid_ref(&self, tile_ref: TileRef) -> bool {
        tile_ref < self.ref_to_x.len()
    }

    pub fn x(&self, tile_ref: TileRef) -> u32 {
        self.ref_to_x[tile_ref]
    }

    pub fn y(&self, tile_ref: TileRef) -> u32 {
        self.ref_to_y[tile_ref]
    }

    pub fn is_valid_coord(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height
    }

    /// Get the MapTile at a specific tile reference
    pub fn tile(&self, tile_ref: TileRef) -> MapTile {
        self.terrain[tile_ref]
    }

    // Terrain getters (immutable) - delegate to MapTile
    pub fn is_land(&self, tile_ref: TileRef) -> bool {
        self.terrain[tile_ref].is_land()
    }

    pub fn is_ocean(&self, tile_ref: TileRef) -> bool {
        self.terrain[tile_ref].is_ocean()
    }

    pub fn is_shoreline(&self, tile_ref: TileRef) -> bool {
        self.terrain[tile_ref].is_shoreline()
    }

    pub fn magnitude(&self, tile_ref: TileRef) -> u8 {
        self.terrain[tile_ref].magnitude()
    }

    pub fn is_water(&self, tile_ref: TileRef) -> bool {
        self.terrain[tile_ref].is_water()
    }

    pub fn is_lake(&self, tile_ref: TileRef) -> bool {
        self.terrain[tile_ref].is_lake()
    }

    pub fn is_shore(&self, tile_ref: TileRef) -> bool {
        self.terrain[tile_ref].is_shore()
    }

    pub fn is_ocean_shore(&self, tile_ref: TileRef) -> bool {
        self.is_land(tile_ref) && self.neighbors(tile_ref).any(|n| self.is_ocean(n))
    }

    pub fn cost(&self, tile_ref: TileRef) -> u8 {
        self.terrain[tile_ref].cost()
    }

    pub fn terrain_type(&self, tile_ref: TileRef) -> TerrainType {
        self.terrain[tile_ref].terrain_type()
    }

    pub fn is_on_edge_of_map(&self, tile_ref: TileRef) -> bool {
        let x = self.x(tile_ref);
        let y = self.y(tile_ref);
        x == 0 || x == self.width - 1 || y == 0 || y == self.height - 1
    }

    /// Get neighbors of a tile (up to 4 neighbors in cardinal directions)
    pub fn neighbors(&self, tile_ref: TileRef) -> impl Iterator<Item = TileRef> + '_ {
        let x = self.ref_to_x[tile_ref];
        let width = self.width as usize;
        let height = self.height as usize;

        [
            // North
            if tile_ref >= width {
                Some(tile_ref - width)
            } else {
                None
            },
            // South
            if tile_ref < ((height - 1) * width) {
                Some(tile_ref + width)
            } else {
                None
            },
            // West
            if x != 0 { Some(tile_ref - 1) } else { None },
            // East
            if x != width as u32 - 1 {
                Some(tile_ref + 1)
            } else {
                None
            },
        ]
        .into_iter()
        .filter_map(|opt| opt)
    }

    /// Manhattan distance between two tiles
    pub fn manhattan_dist(&self, c1: TileRef, c2: TileRef) -> u32 {
        let x1 = self.x(c1);
        let y1 = self.y(c1);
        let x2 = self.x(c2);
        let y2 = self.y(c2);

        x1.abs_diff(x2) + y1.abs_diff(y2)
    }

    /// Squared Euclidean distance between two tiles
    pub fn euclidean_dist_squared(&self, c1: TileRef, c2: TileRef) -> u32 {
        let x1 = self.x(c1) as i64;
        let y1 = self.y(c1) as i64;
        let x2 = self.x(c2) as i64;
        let y2 = self.y(c2) as i64;

        let dx = x1 - x2;
        let dy = y1 - y2;
        ((dx * dx) + (dy * dy)) as u32
    }

    /// Breadth-first search from a tile, filtering by a predicate
    ///
    /// The filter must be applied AOT for performance reasons
    pub fn bfs<'a, F>(&'a self, start: TileRef, mut filter: F) -> impl Iterator<Item = TileRef> + 'a
    where
        F: FnMut(&Self, TileRef) -> bool + 'a,
    {
        let mut seen = vec![false; (self.width * self.height) as usize];

        let mut queue = if filter(self, start) {
            vec![start]
        } else {
            vec![]
        };

        std::iter::from_fn(move || {
            let curr = queue.pop()?;
            for n in self.neighbors(curr).filter(|n| filter(self, *n)) {
                if !seen[n] {
                    seen[n] = true;
                    queue.push(n);
                }
            }
            Some(curr)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_tile_bitfield() {
        // Test bit 7 (is_land)
        let land_tile = MapTile::from_byte(0b10000000);
        assert!(land_tile.is_land());
        assert!(!land_tile.is_water());

        // Test bit 6 (is_shoreline)
        let shore_tile = MapTile::from_byte(0b11000000);
        assert!(shore_tile.is_land());
        assert!(shore_tile.is_shoreline());
        assert!(shore_tile.is_shore());

        // Test bit 5 (is_ocean)
        let ocean_tile = MapTile::from_byte(0b00100000);
        assert!(!ocean_tile.is_land());
        assert!(ocean_tile.is_ocean());
        assert!(ocean_tile.is_water());
        assert_eq!(ocean_tile.terrain_type(), TerrainType::Ocean);

        // Test magnitude (bits 0-4)
        let mountain_tile = MapTile::from_byte(0b10011111); // land + magnitude 31
        assert!(mountain_tile.is_land());
        assert_eq!(mountain_tile.magnitude(), 31);
        assert_eq!(mountain_tile.terrain_type(), TerrainType::Mountain);

        // Test plains
        let plains_tile = MapTile::from_byte(0b10000101); // land + magnitude 5
        assert_eq!(plains_tile.magnitude(), 5);
        assert_eq!(plains_tile.terrain_type(), TerrainType::Plains);

        // Test lake
        let lake_tile = MapTile::from_byte(0b00000000);
        assert!(lake_tile.is_lake());
        assert_eq!(lake_tile.terrain_type(), TerrainType::Lake);
    }

    #[test]
    fn test_map_tile_cost() {
        let plains_tile = MapTile::from_byte(0b10000101); // magnitude 5
        assert_eq!(plains_tile.cost(), 2);

        let mountain_tile = MapTile::from_byte(0b10010100); // magnitude 20
        assert_eq!(mountain_tile.cost(), 1);
    }

    #[test]
    fn test_scan_available_maps() {
        // Just verify the function doesn't crash
        let result = GameMap::scan_available_maps();
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_africa_map() {
        let map = GameMap::load("africa").expect("Failed to load Africa map");
        assert_eq!(map.name(), "Africa");
        assert_eq!(map.width(), 1948);
        assert_eq!(map.height(), 2032);
        assert_eq!(map.num_land_tiles(), 2183186);
        assert!(map.is_water(0));
        assert!(map.is_land(map.tile_ref(1000, 1000).unwrap()));
        assert!(!map.nations().is_empty());
    }

    #[test]
    fn test_coordinate_conversion() {
        let map = GameMap::load("africa").expect("Failed to load Africa map");

        // Test origin
        let origin_ref = map.tile_ref(0, 0).unwrap();
        assert_eq!(origin_ref, 0);
        assert_eq!(map.x(origin_ref), 0);
        assert_eq!(map.y(origin_ref), 0);

        // Test arbitrary coordinate
        let tile_ref = map.tile_ref(100, 50).unwrap();
        assert_eq!(map.x(tile_ref), 100);
        assert_eq!(map.y(tile_ref), 50);

        // Test invalid coordinates
        assert!(map.tile_ref(map.width(), 0).is_none());
        assert!(map.tile_ref(0, map.height()).is_none());
    }

    #[test]
    fn test_neighbors() {
        let map = GameMap::load("africa").expect("Failed to load Africa map");

        // Corner tile (0, 0) should have 2 neighbors
        let corner = map.tile_ref(0, 0).unwrap();
        let neighbors = map.neighbors(corner);
        assert_eq!(neighbors.count(), 2);

        // Edge tile should have 3 neighbors
        let edge = map.tile_ref(1, 0).unwrap();
        let neighbors = map.neighbors(edge);
        assert_eq!(neighbors.count(), 3);

        // Center tile should have 4 neighbors
        let center = map.tile_ref(100, 100).unwrap();
        let neighbors = map.neighbors(center);
        assert_eq!(neighbors.count(), 4);
    }
}
