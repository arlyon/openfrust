enum TerrainType {
    Mountain,
    Plains,
    Sand,
    Forest,
}

/// this will be packed as 2 bytes when loaded into the compute shader
struct Tile {
    terrain: TerrainType, // 2 bits
    elevation: i16,       // +- 8192m
}
