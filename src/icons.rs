//! File icons. GPUI renders SVG assets as alpha masks, so the artwork is built
//! from layered divs; a Lucide glyph marks the content kind.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{
    Hsla, IntoElement, ParentElement, Pixels, Styled, div, linear_color_stop, linear_gradient, px,
    rgba,
};

use crate::model::{EntryKind, FileEntry};

pub fn folder_glyph_color() -> Hsla {
    rgba(0x2f93cdff).into()
}

pub fn mini_folder(width: Pixels) -> impl IntoElement {
    folder_art(width)
}

pub fn folder_icon(entry: &FileEntry, width: Pixels) -> impl IntoElement {
    let height = width * 0.75;
    let glyph_size = width * 0.29;
    div()
        .relative()
        .flex_none()
        .w(width)
        .h(height)
        .child(folder_art(width))
        .child(
            div()
                .absolute()
                .top(height * 0.40)
                .left_0()
                .w_full()
                .h(height * 0.48)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(folder_glyph(entry))
                        .size(glyph_size)
                        .text_color(folder_glyph_color()),
                ),
        )
}

fn folder_art(width: Pixels) -> impl IntoElement {
    let height = width * 0.75;
    let u = width / 96.0;

    div()
        .relative()
        .flex_none()
        .w(width)
        .h(height)
        .child(
            div()
                .absolute()
                .left(8. * u)
                .top(6. * u)
                .w(34. * u)
                .h(16. * u)
                .rounded(6. * u)
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(rgba(0x7fd4f8ff), 0.),
                    linear_color_stop(rgba(0x57b8e9ff), 1.),
                )),
        )
        .child(
            div()
                .absolute()
                .left(4. * u)
                .top(12. * u)
                .w(88. * u)
                .h(58. * u)
                .rounded(9. * u)
                .border_1()
                .border_color(rgba(0x3ba2ddff))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(rgba(0x79d2f9ff), 0.),
                    linear_color_stop(rgba(0x4bb2e8ff), 1.),
                )),
        )
        .child(
            div()
                .absolute()
                .left(3. * u)
                .top(20. * u)
                .w(90. * u)
                .h(50. * u)
                .rounded(9. * u)
                .border_1()
                .border_color(rgba(0x3398d5ff))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(rgba(0x86ddfeff), 0.),
                    linear_color_stop(rgba(0x46b2eeff), 1.),
                )),
        )
        .child(
            div()
                .absolute()
                .left(6. * u)
                .top(23. * u)
                .w(84. * u)
                .h(11. * u)
                .rounded(5. * u)
                .bg(rgba(0xffffff33)),
        )
        .child(
            div()
                .absolute()
                .left(10. * u)
                .top(22. * u)
                .w(76. * u)
                .h(2. * u)
                .rounded(1. * u)
                .bg(rgba(0xe8f9ffb0)),
        )
}

pub fn file_icon(entry: &FileEntry, width: Pixels) -> impl IntoElement {
    let height = width * 1.25;
    let glyph_size = width * 0.38;
    let u = width / 64.0;

    div()
        .relative()
        .flex_none()
        .w(width)
        .h(height)
        .child(
            div()
                .absolute()
                .left(px(0.))
                .top(px(0.))
                .w(width)
                .h(height)
                .rounded(4. * u)
                .border_1()
                .border_color(rgba(0xc5cfdaff))
                .bg(rgba(0xffffffff)),
        )
        .child(
            div()
                .absolute()
                .left(48. * u)
                .top(px(0.))
                .w(16. * u)
                .h(16. * u)
                .rounded(4. * u)
                .bg(rgba(0xeef2f6ff)),
        )
        .child(
            div()
                .absolute()
                .top(height * 0.52)
                .left_0()
                .w_full()
                .h(height * 0.36)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon_for_file(&entry.name))
                        .size(glyph_size)
                        .text_color(rgba(0x64748bff)),
                ),
        )
}

fn folder_glyph(entry: &FileEntry) -> IconName {
    let name = entry.name.to_ascii_lowercase();
    match name.as_str() {
        "pictures" | "photos" | "camera" | "dcim" => IconName::Image,
        "music" | "audio" | "ringtones" | "alarms" | "notifications" | "podcasts" => {
            IconName::Music
        }
        "movies" | "movie" | "videos" | "video" => IconName::Film,
        "download" | "downloads" => IconName::ArrowDownToLine,
        "documents" | "docs" => IconName::FileText,
        "android" | "obb" | "data" => IconName::Package,
        "school" | "classes" => IconName::GraduationCap,
        "public" | "shared" => IconName::Globe,
        _ => IconName::Folder,
    }
}

fn extension(name: &str) -> Option<&str> {
    name.rsplit('.')
        .next()
        .filter(|e| !e.is_empty() && *e != name)
}

fn icon_for_file(name: &str) -> IconName {
    let ext = extension(name).map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "heif" | "bmp" | "svg") => {
            IconName::FileImage
        }
        Some("mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v" | "3gp") => IconName::FileVideoCamera,
        Some("mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "opus") => IconName::FileMusic,
        Some("zip" | "tar" | "gz" | "tgz" | "7z" | "rar" | "xz" | "bz2") => IconName::FileArchive,
        Some("apk" | "aab" | "xapk") => IconName::Package,
        Some("pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "odt" | "log") => IconName::FileText,
        Some(
            "rs" | "js" | "ts" | "tsx" | "jsx" | "py" | "java" | "kt" | "c" | "cpp" | "h" | "hpp"
            | "go" | "swift" | "sh" | "json" | "toml" | "yaml" | "yml" | "css" | "html",
        ) => IconName::FileCode,
        _ => IconName::File,
    }
}

pub fn icon_for_entry(entry: &FileEntry) -> IconName {
    match &entry.kind {
        EntryKind::Dir => IconName::Folder,
        EntryKind::Symlink {
            resolves_to_dir, ..
        } => {
            if *resolves_to_dir {
                IconName::FolderSymlink
            } else {
                IconName::FileSymlink
            }
        }
        EntryKind::File => icon_for_file(&entry.name),
        EntryKind::Other => IconName::File,
    }
}
