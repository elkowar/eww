//! Decoded image files for the `image` widget, kept so that building an image again (reopening its window, or a
//! `for` loop building its element again) does not decode the same file again.

use std::{cell::RefCell, collections::HashMap, os::unix::fs::MetadataExt};

use gtk::gdk_pixbuf::Pixbuf;

/// How much decoded pixel data is kept at most.
const BUDGET_BYTES: usize = 64 * 1024 * 1024;

thread_local! {
    static CACHE: RefCell<PixbufCache> = RefCell::new(PixbufCache::new(BUDGET_BYTES));
}

/// Load the image file at `path` like [`Pixbuf::from_file_at_scale`], reusing an earlier decode of the same file at
/// the same size if the file has not changed since. Pixbufs are immutable once loaded, so images can share them.
pub fn load_at_scale(path: &str, width: i32, height: i32, preserve_aspect_ratio: bool) -> Result<Pixbuf, gtk::glib::Error> {
    let load = || Pixbuf::from_file_at_scale(path, width, height, preserve_aspect_ratio);
    // Without metadata there is nothing to check a cached image against; the decoder reports the error.
    let Ok(metadata) = std::fs::metadata(path) else {
        return load();
    };
    let key = CacheKey { path: path.to_string(), width, height, preserve_aspect_ratio };
    let stamp = FileStamp::of(&metadata);
    if let Some(pixbuf) = CACHE.with(|cache| cache.borrow_mut().get(&key, stamp)) {
        return Ok(pixbuf);
    }
    let pixbuf = load()?;
    CACHE.with(|cache| cache.borrow_mut().insert(key, stamp, pixbuf.clone()));
    Ok(pixbuf)
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    path: String,
    width: i32,
    height: i32,
    preserve_aspect_ratio: bool,
}

/// Identifies the contents a pixbuf was decoded from: replacing the file changes its inode, and writing to it changes
/// its ctime, even when its mtime is set back.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    dev: u64,
    ino: u64,
    size: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

impl FileStamp {
    fn of(metadata: &std::fs::Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            size: metadata.size(),
            mtime: (metadata.mtime(), metadata.mtime_nsec()),
            ctime: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

struct Entry {
    stamp: FileStamp,
    pixbuf: Pixbuf,
    bytes: usize,
    last_use: u64,
}

/// Drops the least recently used entries when their pixel data exceeds the budget.
struct PixbufCache {
    entries: HashMap<CacheKey, Entry>,
    budget: usize,
    bytes: usize,
    uses: u64,
}

impl PixbufCache {
    fn new(budget: usize) -> Self {
        Self { entries: HashMap::new(), budget, bytes: 0, uses: 0 }
    }

    fn get(&mut self, key: &CacheKey, stamp: FileStamp) -> Option<Pixbuf> {
        if self.entries.get(key)?.stamp != stamp {
            self.remove(key);
            return None;
        }
        self.uses += 1;
        let entry = self.entries.get_mut(key)?;
        entry.last_use = self.uses;
        Some(entry.pixbuf.clone())
    }

    fn insert(&mut self, key: CacheKey, stamp: FileStamp, pixbuf: Pixbuf) {
        self.remove(&key);
        let bytes = pixbuf.byte_length();
        if bytes > self.budget {
            return;
        }
        self.uses += 1;
        self.bytes += bytes;
        self.entries.insert(key, Entry { stamp, pixbuf, bytes, last_use: self.uses });
        while self.bytes > self.budget {
            let Some(oldest) = self.entries.iter().min_by_key(|(_, entry)| entry.last_use).map(|(key, _)| key.clone()) else {
                break;
            };
            self.remove(&oldest);
        }
    }

    fn remove(&mut self, key: &CacheKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.bytes -= entry.bytes;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn key(path: &str) -> CacheKey {
        CacheKey { path: path.to_string(), width: -1, height: -1, preserve_aspect_ratio: true }
    }

    fn stamp(ino: u64) -> FileStamp {
        FileStamp { dev: 1, ino, size: 1, mtime: (0, 0), ctime: (0, 0) }
    }

    /// 16x16 RGBA: 1 KiB of pixel data.
    fn pixbuf() -> Pixbuf {
        Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, true, 8, 16, 16).unwrap()
    }

    #[test]
    fn test_evicts_least_recently_used() {
        let mut cache = PixbufCache::new(3 * 1024);
        for path in ["a", "b", "c"] {
            cache.insert(key(path), stamp(1), pixbuf());
        }
        assert!(cache.get(&key("a"), stamp(1)).is_some());
        cache.insert(key("d"), stamp(1), pixbuf());
        assert!(cache.get(&key("b"), stamp(1)).is_none());
        for path in ["a", "c", "d"] {
            assert!(cache.get(&key(path), stamp(1)).is_some());
        }
        assert_eq!(cache.bytes, 3 * 1024);
    }

    #[test]
    fn test_changed_file_is_not_served() {
        let mut cache = PixbufCache::new(3 * 1024);
        cache.insert(key("a"), stamp(1), pixbuf());
        assert!(cache.get(&key("a"), stamp(2)).is_none());
        assert!(cache.get(&key("a"), stamp(1)).is_none());
        assert_eq!(cache.bytes, 0);
    }

    #[test]
    fn test_skips_images_over_budget() {
        let mut cache = PixbufCache::new(512);
        cache.insert(key("a"), stamp(1), pixbuf());
        assert!(cache.entries.is_empty());
        assert_eq!(cache.bytes, 0);
    }
}
