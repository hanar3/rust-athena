use std::collections::{HashSet, VecDeque};
use std::convert::TryInto;
use std::io::{self, Read};
use std::time::Duration;

use flate2::bufread::ZlibDecoder;

pub type Tile = (u16, u16);
const MAX_STEPS: u8 = 14;

pub struct Terrain {
    width: u16,
    height: u16,
    cells: Vec<u8>,
}

impl Terrain {
    /// Read one map from an rAthena map cache. The cache owns the terrain data.
    pub fn from_cache(bytes: &[u8], map: &str) -> io::Result<Self> {
        let header = bytes.get(..8).ok_or_else(invalid_cache)?;
        let size = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
        if size != bytes.len() {
            return Err(invalid_cache());
        }
        let count = u16::from_le_bytes(header[4..6].try_into().unwrap());
        let mut entries = &bytes[8..];
        for _ in 0..count {
            let entry = entries.get(..20).ok_or_else(invalid_cache)?;
            let name = &entry[..12];
            let name_end = name
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(name.len());
            let len = u32::from_le_bytes(entry[16..20].try_into().unwrap()) as usize;
            let compressed = entries[20..].get(..len).ok_or_else(invalid_cache)?;
            entries = &entries[20 + len..];
            if &name[..name_end] != map.as_bytes() {
                continue;
            }

            let width = u16::from_le_bytes(entry[12..14].try_into().unwrap());
            let height = u16::from_le_bytes(entry[14..16].try_into().unwrap());
            if !(2..=1024).contains(&width) || !(2..=1024).contains(&height) {
                return Err(invalid_cache());
            }
            let cell_count = usize::from(width) * usize::from(height);
            let mut cells = Vec::with_capacity(cell_count);
            ZlibDecoder::new(compressed)
                .take(cell_count as u64 + 1)
                .read_to_end(&mut cells)?;
            if cells.len() != cell_count || cells.iter().any(|&cell| cell > 6) {
                return Err(invalid_cache());
            }
            return Ok(Self {
                width,
                height,
                cells,
            });
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("map cache has no {} entry", map),
        ))
    }

    pub fn walkable(&self, (x, y): Tile) -> bool {
        x < self.width - 1
            && y < self.height - 1
            && matches!(
                self.cells[usize::from(y) * usize::from(self.width) + usize::from(x)],
                0 | 2 | 3 | 4 | 6
            )
    }

    pub fn walk_duration(&self, from: Tile, to: Tile) -> Option<Duration> {
        if from == to
            || !self.walkable(from)
            || !self.walkable(to)
            || from.0.abs_diff(to.0).max(from.1.abs_diff(to.1)) > u16::from(MAX_STEPS)
        {
            return None;
        }
        let mut visited = HashSet::from([from]);
        let mut pending = VecDeque::from([(from, 0, 0)]);
        while let Some(((x, y), steps, millis)) = pending.pop_front() {
            if steps == MAX_STEPS {
                continue;
            }
            for (dx, dy) in [
                (0, 1),
                (1, 0),
                (0, -1),
                (-1, 0),
                (1, 1),
                (1, -1),
                (-1, -1),
                (-1, 1),
            ] {
                let (Some(next_x), Some(next_y)) =
                    (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    continue;
                };
                let diagonal = dx != 0 && dy != 0;
                let next = (next_x, next_y);
                if !self.walkable(next)
                    || (diagonal && (!self.walkable((next_x, y)) || !self.walkable((x, next_y))))
                    || !visited.insert(next)
                {
                    continue;
                }
                let arrival = millis + if diagonal { 210 } else { 150 };
                if next == to {
                    return Some(Duration::from_millis(arrival));
                }
                pending.push_back((next, steps + 1, arrival));
            }
        }
        None
    }
}

fn invalid_cache() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid rAthena map cache")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    #[test]
    fn cached_terrain_rejects_walls_and_disallows_cutting_corners() {
        let mut cells = [0; 16];
        cells[1] = 1;
        cells[4] = 5;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&cells).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut cache = Vec::new();
        cache.extend(((28 + compressed.len()) as u32).to_le_bytes());
        cache.extend([1, 0, 0, 0]);
        cache.extend(b"prontera\0\0\0\0");
        cache.extend([4, 0, 4, 0]);
        cache.extend((compressed.len() as u32).to_le_bytes());
        cache.extend(compressed);

        let map = Terrain::from_cache(&cache, "prontera").unwrap();
        assert_eq!(
            map.walk_duration((1, 1), (2, 1)),
            Some(Duration::from_millis(150))
        );
        assert_eq!(map.walk_duration((1, 1), (0, 1)), None);
        assert_eq!(map.walk_duration((1, 1), (0, 0)), None);
        assert_eq!(map.walk_duration((1, 1), (3, 0)), None);
        assert!(Terrain::from_cache(&cache[..cache.len() - 1], "prontera").is_err());
    }
}
