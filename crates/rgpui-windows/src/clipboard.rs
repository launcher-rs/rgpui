use std::sync::LazyLock;

use anyhow::Result;
use itertools::Itertools;
use rgpui::collections::FxHashMap;
use windows::Win32::{
    Foundation::{HANDLE, HGLOBAL},
    System::{
        DataExchange::{
            CloseClipboard, CountClipboardFormats, EmptyClipboard, EnumClipboardFormats,
            GetClipboardData, GetClipboardFormatNameW, OpenClipboard, RegisterClipboardFormatW,
            SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT},
    },
    UI::Shell::{DragQueryFileW, HDROP},
};
use windows::core::{Owned, PCWSTR};

use rgpui::{
    ClipboardEntry, ClipboardFormat, ClipboardItem, ClipboardString, ExternalPaths, Image,
    ImageFormat, hash,
};

const DRAGDROP_GET_FILES_COUNT: u32 = 0xFFFFFFFF;

/// rgpui 会在 Windows 剪贴板上读写的富负载格式（注册格式名 ↔ MIME 类型）。
///
/// 「HTML Format」是 Windows 的事实标准（`CF_HTML`），别名 `text/html`；
/// 其余名称与 MIME 同名，可直接注册。
const RICH_FORMATS: &[(&str, &str)] = &[
    ("HTML Format", ClipboardFormat::HTML),
    ("Rich Text Format", ClipboardFormat::RTF),
    ("text/csv", ClipboardFormat::CSV),
    ("text/markdown", ClipboardFormat::MARKDOWN),
    ("text/tab-separated-values", ClipboardFormat::TSV),
    ("text/uri-list", ClipboardFormat::URI_LIST),
];

/// 富负载格式名 → 注册的 Windows 剪贴板格式 id。
static RICH_FORMAT_IDS: LazyLock<Vec<(u32, &'static str)>> = LazyLock::new(|| {
    RICH_FORMATS
        .iter()
        .map(|(name, mime)| (register_clipboard_format_str(name), *mime))
        .collect()
});

static CLIPBOARD_HASH_FORMAT: LazyLock<u32> =
    LazyLock::new(|| register_clipboard_format(windows::core::w!("GPUI internal text hash")));
static CLIPBOARD_METADATA_FORMAT: LazyLock<u32> =
    LazyLock::new(|| register_clipboard_format(windows::core::w!("GPUI internal metadata")));
static CLIPBOARD_SVG_FORMAT: LazyLock<u32> =
    LazyLock::new(|| register_clipboard_format(windows::core::w!("image/svg+xml")));
static CLIPBOARD_GIF_FORMAT: LazyLock<u32> =
    LazyLock::new(|| register_clipboard_format(windows::core::w!("GIF")));
static CLIPBOARD_PNG_FORMAT: LazyLock<u32> =
    LazyLock::new(|| register_clipboard_format(windows::core::w!("PNG")));
static CLIPBOARD_JPG_FORMAT: LazyLock<u32> =
    LazyLock::new(|| register_clipboard_format(windows::core::w!("JFIF")));

static IMAGE_FORMATS_MAP: LazyLock<FxHashMap<u32, ImageFormat>> = LazyLock::new(|| {
    let mut map = FxHashMap::default();
    map.insert(*CLIPBOARD_PNG_FORMAT, ImageFormat::Png);
    map.insert(*CLIPBOARD_GIF_FORMAT, ImageFormat::Gif);
    map.insert(*CLIPBOARD_JPG_FORMAT, ImageFormat::Jpeg);
    map.insert(*CLIPBOARD_SVG_FORMAT, ImageFormat::Svg);
    map
});

fn register_clipboard_format(format: PCWSTR) -> u32 {
    let ret = unsafe { RegisterClipboardFormatW(format) };
    if ret == 0 {
        panic!(
            "Error when registering clipboard format: {}",
            std::io::Error::last_os_error()
        );
    }
    log::debug!(
        "Registered clipboard format {} as {}",
        unsafe { format.display() },
        ret
    );
    ret
}

/// 按格式名（运行时字符串）注册剪贴板格式。
fn register_clipboard_format_str(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    register_clipboard_format(PCWSTR(wide.as_ptr()))
}

/// 查富负载 MIME 类型对应的 Windows 注册格式 id（含 HTML/RTF 的 Windows 别名）。
fn rich_format_id(mime_type: &str) -> u32 {
    RICH_FORMAT_IDS
        .iter()
        .find(|(_, mime)| *mime == mime_type)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| register_clipboard_format_str(mime_type))
}

fn get_clipboard_data(format: u32) -> Option<LockedGlobal> {
    let global = HGLOBAL(unsafe { GetClipboardData(format).ok() }?.0);
    LockedGlobal::lock(global)
}

pub(crate) fn write_to_clipboard(item: ClipboardItem) {
    let Some(_clip) = ClipboardGuard::open() else {
        return;
    };

    let result: Result<()> = (|| {
        unsafe { EmptyClipboard()? };
        for entry in item.entries() {
            match entry {
                ClipboardEntry::String(string) => write_string(string)?,
                ClipboardEntry::Image(image) => write_image(image)?,
                ClipboardEntry::ExternalPaths(_) => {}
                ClipboardEntry::Format(format) => write_format(format)?,
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        log::error!("Failed to write to clipboard: {e}");
    }
}

pub(crate) fn read_from_clipboard() -> Option<ClipboardItem> {
    let _clip = ClipboardGuard::open()?;

    let mut entries = Vec::new();
    let mut have_text = false;
    let mut have_image = false;
    let mut have_files = false;

    let count = unsafe { CountClipboardFormats() };
    let mut format = 0;
    for _ in 0..count {
        format = unsafe { EnumClipboardFormats(format) };

        if !have_text && format == CF_UNICODETEXT.0 as u32 {
            if let Some(entry) = read_string() {
                entries.push(entry);
                have_text = true;
            }
        } else if !have_image && is_image_format(format) {
            if let Some(entry) = read_image(format) {
                entries.push(entry);
                have_image = true;
            }
        } else if !have_files && format == CF_HDROP.0 as u32 {
            if let Some(entry) = read_files() {
                entries.push(entry);
                have_files = true;
            }
        }
    }

    // 富负载（HTML / RTF / CSV…）统一排在纯文本与图像之后，读取顺序稳定。
    for (id, mime_type) in RICH_FORMAT_IDS.iter() {
        if let Some(entry) = read_rich_format(*id, mime_type) {
            entries.push(entry);
        }
    }

    if entries.is_empty() {
        log_unsupported_clipboard_formats();
        return None;
    }
    Some(ClipboardItem { entries })
}

/// 写入一份富负载；`text/html` 需按 `CF_HTML` 规范补全文档外壳。
fn write_format(format: &ClipboardFormat) -> Result<()> {
    let id = rich_format_id(&format.mime_type);
    if format.mime_type == ClipboardFormat::HTML {
        let wrapped = wrap_cf_html(format.text().unwrap_or_default());
        set_clipboard_bytes(&wrapped, id)?;
    } else {
        set_clipboard_bytes(&format.bytes, id)?;
    }
    Ok(())
}

/// 读取一份富负载；HTML 按 `CF_HTML` 规范解出片段。
fn read_rich_format(id: u32, mime_type: &str) -> Option<ClipboardEntry> {
    let locked = get_clipboard_data(id)?;
    let bytes = locked.as_bytes();
    let bytes = if mime_type == ClipboardFormat::HTML {
        unwrap_cf_html(bytes)?.to_vec()
    } else {
        bytes.to_vec()
    };
    Some(ClipboardEntry::Format(ClipboardFormat::new(
        mime_type, bytes,
    )))
}

/// 片段前的固定标记（写入时使用，`StartFragment` 指向它之后）。
const CF_HTML_PREFIX: &str = "<html><body><!--StartFragment-->";
/// 片段后的固定标记。
const CF_HTML_SUFFIX: &str = "<!--EndFragment--></body></html>";

/// 把裸 HTML 片段包成 Windows 的 `CF_HTML`（"HTML Format"）文档。
fn wrap_cf_html(fragment: &str) -> Vec<u8> {
    /// 渲染 `CF_HTML` 头：四个偏移量固定 10 位十进制，头部字节长度与取值无关。
    fn render(
        start_html: usize,
        end_html: usize,
        start_fragment: usize,
        end_fragment: usize,
    ) -> String {
        format!(
            "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\n\
StartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n"
        )
    }

    let start_html = render(0, 0, 0, 0).len();
    let start_fragment = start_html + CF_HTML_PREFIX.len();
    let end_fragment = start_fragment + fragment.len();
    let end_html = end_fragment + CF_HTML_SUFFIX.len();

    let mut out = render(start_html, end_html, start_fragment, end_fragment).into_bytes();
    out.extend_from_slice(CF_HTML_PREFIX.as_bytes());
    out.extend_from_slice(fragment.as_bytes());
    out.extend_from_slice(CF_HTML_SUFFIX.as_bytes());
    out
}

/// 从 `CF_HTML` 文档中解出片段；缺少标记时按整段返回。
fn unwrap_cf_html(bytes: &[u8]) -> Option<&[u8]> {
    let text = std::str::from_utf8(bytes).ok()?;
    let start = cf_html_offset(text, "StartFragment:")?;
    let end = cf_html_offset(text, "EndFragment:")?;
    if start > end || end > bytes.len() {
        return None;
    }
    Some(&bytes[start..end])
}

/// 解析 `CF_HTML` 头中某个偏移量字段（紧跟在字段名后的十进制数字）。
fn cf_html_offset(header: &str, field: &str) -> Option<usize> {
    let rest = &header[header.find(field)? + field.len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

pub(crate) fn with_file_names<F>(hdrop: HDROP, mut f: F)
where
    F: FnMut(String),
{
    let file_count = unsafe { DragQueryFileW(hdrop, DRAGDROP_GET_FILES_COUNT, None) };
    for file_index in 0..file_count {
        let filename_length = unsafe { DragQueryFileW(hdrop, file_index, None) } as usize;
        let mut buffer = vec![0u16; filename_length + 1];
        let ret = unsafe { DragQueryFileW(hdrop, file_index, Some(buffer.as_mut_slice())) };
        if ret == 0 {
            log::error!("unable to read file name of dragged file");
            continue;
        }
        match String::from_utf16(&buffer[0..filename_length]) {
            Ok(file_name) => f(file_name),
            Err(e) => log::error!("dragged file name is not UTF-16: {}", e),
        }
    }
}

fn set_clipboard_bytes<T>(data: &[T], format: u32) -> Result<()> {
    unsafe {
        let global = Owned::new(GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of_val(data))?);
        let ptr = GlobalLock(*global);
        anyhow::ensure!(!ptr.is_null(), "GlobalLock returned null");
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as _, data.len());
        GlobalUnlock(*global).ok();
        SetClipboardData(format, Some(HANDLE(global.0)))?;
        // SetClipboardData succeeded — the system now owns the memory.
        std::mem::forget(global);
    }
    Ok(())
}

fn get_clipboard_string(format: u32) -> Option<String> {
    let locked = get_clipboard_data(format)?;
    let bytes = locked.as_bytes();
    let words_len = bytes.len() / std::mem::size_of::<u16>();
    if words_len == 0 {
        return Some(String::new());
    }
    let slice = unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u16, words_len) };
    let actual_len = slice.iter().position(|&c| c == 0).unwrap_or(words_len);
    Some(String::from_utf16_lossy(&slice[..actual_len]))
}

fn is_image_format(format: u32) -> bool {
    IMAGE_FORMATS_MAP.contains_key(&format) || format == CF_DIB.0 as u32
}

fn write_string(item: &ClipboardString) -> Result<()> {
    let wide: Vec<u16> = item.text.encode_utf16().chain(Some(0)).collect_vec();
    set_clipboard_bytes(&wide, CF_UNICODETEXT.0 as u32)?;

    if let Some(metadata) = item.metadata.as_ref() {
        let hash_bytes = ClipboardString::text_hash(&item.text).to_ne_bytes();
        set_clipboard_bytes(&hash_bytes, *CLIPBOARD_HASH_FORMAT)?;

        let wide: Vec<u16> = metadata.encode_utf16().chain(Some(0)).collect_vec();
        set_clipboard_bytes(&wide, *CLIPBOARD_METADATA_FORMAT)?;
    }
    Ok(())
}

fn write_image(item: &Image) -> Result<()> {
    let native_format = match item.format {
        ImageFormat::Svg => Some(*CLIPBOARD_SVG_FORMAT),
        ImageFormat::Gif => Some(*CLIPBOARD_GIF_FORMAT),
        ImageFormat::Png => Some(*CLIPBOARD_PNG_FORMAT),
        ImageFormat::Jpeg => Some(*CLIPBOARD_JPG_FORMAT),
        _ => None,
    };
    if let Some(format) = native_format {
        set_clipboard_bytes(item.bytes(), format)?;
    }

    // Also provide a PNG copy for broad compatibility.
    // SVG can't be rasterized by the image crate, so skip it.
    if item.format != ImageFormat::Svg && native_format != Some(*CLIPBOARD_PNG_FORMAT) {
        if let Some(png_bytes) = convert_to_png(item.bytes(), item.format) {
            set_clipboard_bytes(&png_bytes, *CLIPBOARD_PNG_FORMAT)?;
        }
    }
    Ok(())
}

fn convert_to_png(bytes: &[u8], format: ImageFormat) -> Option<Vec<u8>> {
    let img_format = gpui_to_image_format(format)?;
    let image = image::load_from_memory_with_format(bytes, img_format)
        .map_err(|e| log::warn!("Failed to decode image for PNG conversion: {e}"))
        .ok()?;
    let mut buf = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| log::warn!("Failed to encode PNG: {e}"))
        .ok()?;
    Some(buf)
}

fn read_string() -> Option<ClipboardEntry> {
    let text = get_clipboard_string(CF_UNICODETEXT.0 as u32)?;
    let metadata = read_clipboard_metadata(&text);
    Some(ClipboardEntry::String(ClipboardString { text, metadata }))
}

fn read_clipboard_metadata(text: &str) -> Option<String> {
    let locked = get_clipboard_data(*CLIPBOARD_HASH_FORMAT)?;
    let hash_bytes: [u8; 8] = locked.as_bytes().get(..8)?.try_into().ok()?;
    let hash = u64::from_ne_bytes(hash_bytes);
    if hash != ClipboardString::text_hash(text) {
        return None;
    }
    get_clipboard_string(*CLIPBOARD_METADATA_FORMAT)
}

fn read_image(format: u32) -> Option<ClipboardEntry> {
    let locked = get_clipboard_data(format)?;
    let (bytes, image_format) = if format == CF_DIB.0 as u32 {
        (convert_dib_to_bmp(locked.as_bytes())?, ImageFormat::Bmp)
    } else {
        let image_format = *IMAGE_FORMATS_MAP.get(&format)?;
        (locked.as_bytes().to_vec(), image_format)
    };
    let id = hash(&bytes);
    Some(ClipboardEntry::Image(Image {
        format: image_format,
        bytes,
        id,
    }))
}

fn read_files() -> Option<ClipboardEntry> {
    let locked = get_clipboard_data(CF_HDROP.0 as u32)?;
    let hdrop = HDROP(locked.ptr as *mut _);
    let mut filenames = Vec::new();
    with_file_names(hdrop, |name| filenames.push(std::path::PathBuf::from(name)));
    Some(ClipboardEntry::ExternalPaths(ExternalPaths(
        filenames.into(),
    )))
}

/// DIB is BMP without the 14-byte BITMAPFILEHEADER. Prepend one.
fn convert_dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }

    let header_size = u32::from_le_bytes(dib[0..4].try_into().ok()?);
    let bit_count = u16::from_le_bytes(dib[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);

    let color_table_size = if bit_count <= 8 {
        let colors_used = u32::from_le_bytes(dib[32..36].try_into().ok()?);
        (if colors_used == 0 {
            1u32 << bit_count
        } else {
            colors_used
        }) * 4
    } else if compression == 3 {
        12 // BI_BITFIELDS
    } else {
        0
    };

    let pixel_offset = 14 + header_size + color_table_size;
    let file_size = 14 + dib.len() as u32;

    let mut bmp = Vec::with_capacity(file_size as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]); // reserved
    bmp.extend_from_slice(&pixel_offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

fn log_unsupported_clipboard_formats() {
    let count = unsafe { CountClipboardFormats() };
    let mut format = 0;
    for _ in 0..count {
        format = unsafe { EnumClipboardFormats(format) };
        let mut buffer = [0u16; 64];
        unsafe { GetClipboardFormatNameW(format, &mut buffer) };
        let format_name = String::from_utf16_lossy(&buffer);
        log::warn!(
            "Try to paste with unsupported clipboard format: {}, {}.",
            format,
            format_name
        );
    }
}

fn gpui_to_image_format(value: ImageFormat) -> Option<image::ImageFormat> {
    match value {
        ImageFormat::Png => Some(image::ImageFormat::Png),
        ImageFormat::Jpeg => Some(image::ImageFormat::Jpeg),
        ImageFormat::Webp => Some(image::ImageFormat::WebP),
        ImageFormat::Gif => Some(image::ImageFormat::Gif),
        ImageFormat::Bmp => Some(image::ImageFormat::Bmp),
        ImageFormat::Tiff => Some(image::ImageFormat::Tiff),
        other => {
            log::warn!("No image crate equivalent for format: {other:?}");
            None
        }
    }
}

struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> Option<Self> {
        match unsafe { OpenClipboard(None) } {
            Ok(()) => Some(Self),
            Err(e) => {
                log::error!("Failed to open clipboard: {e}");
                None
            }
        }
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        if let Err(e) = unsafe { CloseClipboard() } {
            log::error!("Failed to close clipboard: {e}");
        }
    }
}

struct LockedGlobal {
    global: HGLOBAL,
    ptr: *const u8,
    size: usize,
}

impl LockedGlobal {
    fn lock(global: HGLOBAL) -> Option<Self> {
        let size = unsafe { GlobalSize(global) };
        let ptr = unsafe { GlobalLock(global) };
        if ptr.is_null() {
            return None;
        }
        Some(Self {
            global,
            ptr: ptr as *const u8,
            size,
        })
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.size) }
    }
}

impl Drop for LockedGlobal {
    fn drop(&mut self) {
        unsafe { GlobalUnlock(self.global).ok() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cf_html_round_trip() {
        let fragment = "<b>hello</b> &amp; <i>world</i>";
        let wrapped = wrap_cf_html(fragment);
        let text = std::str::from_utf8(&wrapped).unwrap();
        assert!(text.starts_with("Version:0.9\r\n"));
        assert_eq!(unwrap_cf_html(&wrapped).unwrap(), fragment.as_bytes());
    }

    #[test]
    fn cf_html_offsets_are_consistent() {
        let wrapped = wrap_cf_html("abc");
        let text = std::str::from_utf8(&wrapped).unwrap();
        let start_html = cf_html_offset(text, "StartHTML:").unwrap();
        let start_fragment = cf_html_offset(text, "StartFragment:").unwrap();
        let end_fragment = cf_html_offset(text, "EndFragment:").unwrap();
        assert_eq!(&text[start_html..start_html + 6], "<html>");
        assert_eq!(&wrapped[start_fragment..end_fragment], b"abc");
    }

    #[test]
    fn cf_html_missing_markers_falls_back_to_none() {
        assert!(unwrap_cf_html(b"no markers here").is_none());
    }
}
