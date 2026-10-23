//! What a message carries besides its text: files and folders, held as their
//! paths, never their bytes, in the order they were added. Sending turns an
//! image into an image block and anything else into its path in the text.

use std::path::{Path, PathBuf};

/// Most attachments one message holds.
pub const LIMIT: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
    /// A PNG, JPEG, GIF or WebP, by its extension: what an agent takes as an
    /// image.
    Image,
    File,
    Folder,
}

impl AttachmentKind {
    /// Decided once, when the path is attached: a folder by what is on disk,
    /// an image by its extension, anything else a file.
    fn of(path: &Path) -> Self {
        if path.is_dir() {
            return Self::Folder;
        }
        let image = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                ["png", "jpg", "jpeg", "gif", "webp"]
                    .iter()
                    .any(|image| extension.eq_ignore_ascii_case(image))
            });
        if image { Self::Image } else { Self::File }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub path: PathBuf,
    pub kind: AttachmentKind,
}

impl Attachment {
    /// The file or folder's own name, or the whole path when it has none.
    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

/// A message's attachments: each path once, in the order added, at most
/// `LIMIT`.
#[derive(Debug, Default)]
pub struct Attachments(Vec<Attachment>);

impl Attachments {
    /// Adds each path not attached yet, in order, while there is room, and
    /// returns how many new ones there was no room for.
    pub fn add(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        let mut refused = 0;
        for path in paths {
            if self.0.iter().any(|attached| attached.path == path) {
                continue;
            }
            if self.0.len() == LIMIT {
                refused += 1;
                continue;
            }
            let kind = AttachmentKind::of(&path);
            self.0.push(Attachment { path, kind });
        }
        refused
    }

    pub fn remove(&mut self, path: &Path) {
        self.0.retain(|attached| attached.path != path);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Attachment> {
        self.0.iter()
    }
}

/// What the composer says when `refused` paths found no room.
pub fn refused_text(refused: usize) -> String {
    let count = match refused {
        1 => "1 wasn't".to_owned(),
        _ => format!("{refused} weren't"),
    };
    format!("A message takes up to {LIMIT} attachments, so {count} added.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(attachments: &Attachments) -> Vec<&Path> {
        attachments.iter().map(|a| a.path.as_path()).collect()
    }

    #[test]
    fn a_path_attaches_once_in_the_order_first_added() {
        let mut attachments = Attachments::default();
        let refused = attachments.add(["/a.txt", "/b.txt", "/a.txt"].map(PathBuf::from));
        attachments.add([PathBuf::from("/b.txt")]);
        assert_eq!(refused, 0);
        assert_eq!(
            paths(&attachments),
            [Path::new("/a.txt"), Path::new("/b.txt")]
        );
    }

    #[test]
    fn past_the_limit_new_paths_are_refused_and_counted() {
        let mut attachments = Attachments::default();
        let many = (0..LIMIT + 3).map(|i| PathBuf::from(format!("/{i}.txt")));
        assert_eq!(attachments.add(many), 3);
        assert_eq!(attachments.iter().count(), LIMIT);
        // One already attached is no new one, so it isn't refused.
        assert_eq!(attachments.add([PathBuf::from("/0.txt")]), 0);
        attachments.remove(Path::new("/5.txt"));
        assert_eq!(attachments.add([PathBuf::from("/new.txt")]), 0);
        assert_eq!(
            attachments.iter().last().unwrap().path,
            Path::new("/new.txt")
        );
    }

    #[test]
    fn the_kind_comes_from_the_disk_or_the_extension() {
        let folder = std::env::temp_dir();
        let mut attachments = Attachments::default();
        attachments.add([
            folder.clone(),
            PathBuf::from("/shot.PNG"),
            PathBuf::from("/photo.jpeg"),
            PathBuf::from("/scan.tiff"),
            PathBuf::from("/notes"),
        ]);
        let kinds: Vec<_> = attachments.iter().map(|a| a.kind).collect();
        use AttachmentKind::*;
        assert_eq!(kinds, [Folder, Image, Image, File, File]);
    }

    #[test]
    fn the_refusal_counts_in_plain_words() {
        assert_eq!(
            refused_text(1),
            "A message takes up to 20 attachments, so 1 wasn't added."
        );
        assert_eq!(
            refused_text(4),
            "A message takes up to 20 attachments, so 4 weren't added."
        );
    }
}
