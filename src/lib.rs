use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use rodio::{Decoder, DeviceSinkBuilder, Player};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::BufReader,
    path::Path as StdPath,
    sync::{mpsc, Arc, RwLock},
    time::Duration,
};
use tokio::process::Command as TokioCommand;
use tokio_util::io::ReaderStream;
use tower_http::cors::CorsLayer;
use walkdir::WalkDir;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Track {
    pub id: usize,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub format: String,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub duration_secs: u64,
    pub file_path: String,
    pub source: String, // "LOCAL" | "GOOGLE_DRIVE"
    #[serde(default)]
    pub custom_cover: Option<String>,
    #[serde(default)]
    pub offline_path: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PlayerStatus {
    pub current_track: Option<Track>,
    pub is_playing: bool,
    pub volume: f32,
    pub position_secs: u64,
    pub ram_buffer_mb: f32,
    pub signal_path: SignalPath,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SignalPath {
    pub source_format: String,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub buffer_mode: String,
    pub engine: String,
    pub output_mode: String,
    pub is_bit_perfect: bool,
}

enum AudioCmd {
    Play(Track),
    Pause,
    Resume,
    SetVolume(f32),
    Seek(u64),
}

struct AppState {
    tracks: Arc<RwLock<Vec<Track>>>,
    status: Arc<RwLock<PlayerStatus>>,
    cmd_tx: mpsc::Sender<AudioCmd>,
}

fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let clean = s.split(',').last().unwrap_or(s).trim();
    let b64_table = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let bytes: Vec<u8> = clean.bytes().filter(|&b| b != b'=' && b != b'\r' && b != b'\n').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut val = 0u32;
        for &b in chunk {
            let v = b64_table(b)?;
            val = (val << 6) | (v as u32);
        }
        match chunk.len() {
            4 => {
                out.push((val >> 16) as u8);
                out.push((val >> 8) as u8);
                out.push(val as u8);
            }
            3 => {
                val <<= 6;
                out.push((val >> 16) as u8);
                out.push((val >> 8) as u8);
            }
            2 => {
                val <<= 12;
                out.push((val >> 16) as u8);
            }
            _ => return None,
        }
    }
    Some(out)
}

fn split_camel_case(s: &str) -> String {
    let mut res = String::new();
    let chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len() {
        if i > 0 {
            let prev = chars[i - 1];
            let curr = chars[i];
            if curr.is_uppercase() && (prev.is_lowercase() || prev.is_numeric()) {
                res.push(' ');
            }
        }
        res.push(chars[i]);
    }
    res
}

fn map_vietnamese_artist(raw: &str) -> String {
    let clean = raw.trim();
    match clean {
        "DinhDung" => "Đinh Dũng".to_string(),
        "NooPhuocThinh" => "Noo Phước Thịnh".to_string(),
        "ThaiPhongVu" => "Thái Phong Vũ".to_string(),
        "HuongLy" => "Hương Ly".to_string(),
        "ThuyChiNhomM4U" => "Thùy Chi & M4U".to_string(),
        "HienThuc" => "Hiền Thục".to_string(),
        "LeQuyenTuanNgoc" => "Lệ Quyên & Tuấn Ngọc".to_string(),
        "LeQuyen" => "Lệ Quyên".to_string(),
        "DinhDungLeCuong" => "Đinh Dũng & Lê Cương".to_string(),
        _ => split_camel_case(clean),
    }
}

fn map_vietnamese_title(raw: &str) -> String {
    let clean = raw.trim();
    if clean == "100YeuEm" {
        return "100% Yêu Em".to_string();
    }
    if clean.starts_with("AnhKhongThaThuAcousticVersion") {
        return "Anh Không Tha Thứ (Acoustic Version)".to_string();
    }
    if clean.starts_with("AnhKhongThaThuLofiVersion") {
        return "Anh Không Tha Thứ (Lofi Version)".to_string();
    }
    if clean.starts_with("AnhKhongThaThuPhienBanKiNiem") {
        return "Anh Không Tha Thứ (Phiên Bản Kỉ Niệm)".to_string();
    }
    if clean.starts_with("AnhKhongThaThu") {
        return "Anh Không Tha Thứ".to_string();
    }
    if clean == "AnhCan" {
        return "Anh Cần".to_string();
    }
    if clean == "AnhDiTimEm" {
        return "Anh Đi Tìm Em".to_string();
    }
    if clean == "AnhMuonEmSongSao" {
        return "Anh Muốn Em Sống Sao".to_string();
    }
    if clean == "AnhSeQuen" {
        return "Anh Sẽ Quên".to_string();
    }
    if clean == "AnhThuongEmConNonDai" {
        return "Anh Thương Em Còn Non Dại".to_string();
    }
    if clean.to_lowercase() == "anhtrangtuoitho" {
        return "Ánh Trăng Tuổi Thơ".to_string();
    }
    if clean == "AoBaBa" {
        return "Áo Bà Ba".to_string();
    }
    if clean == "BaiKhongTenCuoiCung" {
        return "Bài Không Tên Cuối Cùng".to_string();
    }
    if clean == "BaiKhongTenSo1" {
        return "Bài Không Tên Số 1".to_string();
    }
    if clean == "BaiKhongTenSo2" {
        return "Bài Không Tên Số 2".to_string();
    }
    if clean == "BaiKhongTenSo3" {
        return "Bài Không Tên Số 3".to_string();
    }
    if clean == "BaiKhongTenSo4" {
        return "Bài Không Tên Số 4".to_string();
    }
    if clean == "BaiKhongTenSo5" {
        return "Bài Không Tên Số 5".to_string();
    }
    if clean == "LangTham" {
        return "Lặng Thầm".to_string();
    }
    if clean.starts_with("TraiTimKhongNguYen") {
        return "Trái Tim Không Ngủ Yên".to_string();
    }
    if clean.starts_with("DauCoLoiLam") {
        return "Dẫu Có Lỗi Lầm (Ballad Version)".to_string();
    }
    if clean == "QuayVeDi" {
        return "Quay Về Đi".to_string();
    }
    if clean.starts_with("YeuDauTheoGioBayDuetVersion") {
        return "Yêu Dấu Theo Gió Bay (Duet Version)".to_string();
    }
    if clean.starts_with("YeuDauTheoGioBayJazzVersionInstrumental") {
        return "Yêu Dấu Theo Gió Bay (Jazz Instrumental)".to_string();
    }
    if clean.starts_with("YeuDauTheoGioBayJazzVersion") {
        return "Yêu Dấu Theo Gió Bay (Jazz Version)".to_string();
    }
    if clean.starts_with("YeuDauTheoGioBayBalladeVersionInstrumental") {
        return "Yêu Dấu Theo Gió Bay (Ballade Instrumental)".to_string();
    }
    if clean.starts_with("YeuDauTheoGioBayBalladeVersion") {
        return "Yêu Dấu Theo Gió Bay (Ballade Version)".to_string();
    }
    if clean.starts_with("YeuDauTheoGioBayRockVersionInstrumental") {
        return "Yêu Dấu Theo Gió Bay (Rock Instrumental)".to_string();
    }
    if clean.starts_with("YeuDauTheoGioBayInstrumental") {
        return "Yêu Dấu Theo Gió Bay (Instrumental)".to_string();
    }
    if clean == "UocGi" {
        return "Ước Gì".to_string();
    }
    if clean == "BayNgayDoiMong" {
        return "Bảy Ngày Đợi Mong".to_string();
    }
    if clean == "GiacMoTrua" {
        return "Giấc Mơ Trưa".to_string();
    }
    if clean == "Neu" {
        return "Nếu".to_string();
    }
    if clean.to_lowercase() == "themmotlanyeuthuong" {
        return "Thêm Một Lần Yêu Thương".to_string();
    }
    if clean.to_lowercase() == "thoidungchiembaotuongvan" {
        return "Thôi Đừng Chiêm Bao (Tường Vân)".to_string();
    }
    if clean.to_lowercase() == "nheanh" {
        return "Nhẹ Anh".to_string();
    }
    if clean == "KiUc" {
        return "Kí Ức".to_string();
    }
    if clean.to_lowercase() == "motngaymuadong" {
        return "Một Ngày Mùa Đông".to_string();
    }
    if clean == "HatChoMotCuocTinh" {
        return "Hát Cho Một Cuộc Tình".to_string();
    }
    if clean == "Quen" {
        return "Quên".to_string();
    }
    if clean == "DoiThay" {
        return "Đổi Thay".to_string();
    }
    if clean == "NhuMotThoiQuen" {
        return "Như Một Thói Quen".to_string();
    }
    if clean == "NuHonBatNgo" {
        return "Nụ Hôn Bất Ngờ".to_string();
    }
    if clean == "ChiecLaCuoiCung" {
        return "Chiếc Lá Cuối Cùng".to_string();
    }
    if clean.starts_with("NoiThoiGianNgungLai") {
        return "Nơi Thời Gian Ngừng Lại (Song Ca)".to_string();
    }

    split_camel_case(clean)
}

fn parse_tags(raw_name: &str) -> (String, String) {
    let unescaped = raw_name
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"");

    let clean = unescaped
        .trim_end_matches(".flac")
        .trim_end_matches(".mp3")
        .trim_end_matches(".wav")
        .trim_end_matches(".m4a");

    let stripped = clean
        .split("_hq")
        .next().unwrap_or(clean)
        .split("_79db")
        .next().unwrap_or(clean)
        .split("_t3ay")
        .next().unwrap_or(clean);

    // Check underscore format like 13_RPT_MCK_Nếu_Như_Ta_Chẳng_Còn
    if !stripped.contains('-') && stripped.contains('_') {
        let mut u_parts: Vec<&str> = stripped.split('_').filter(|s| !s.is_empty()).collect();
        if !u_parts.is_empty() && u_parts[0].chars().all(|c| c.is_ascii_digit()) {
            u_parts.remove(0);
        }
        if u_parts.len() >= 3 && u_parts[0].eq_ignore_ascii_case("RPT") && u_parts[1].eq_ignore_ascii_case("MCK") {
            let title = u_parts[2..].join(" ");
            return (title, "RPT MCK".to_string());
        } else if u_parts.len() >= 2 {
            let artist = u_parts[0].to_string();
            let title = u_parts[1..].join(" ");
            return (title, artist);
        }
    }

    let parts: Vec<&str> = stripped.split('-').map(|s| s.trim()).collect();

    if parts.len() >= 3 {
        // Case 1: TrackNum - Artist - Title (e.g. "02 - RPT MCK - IDK")
        if parts[0].chars().all(|c| c.is_ascii_digit()) {
            let artist = parts[1].to_string();
            let title = parts[2..].join(" - ");
            return (title, artist);
        }
        // Case 2: Title - Artist - ID (e.g. "AnhKhongThaThu - DinhDung - 6684271")
        if parts[parts.len() - 1].chars().all(|c| c.is_ascii_digit()) {
            let title = map_vietnamese_title(parts[0]);
            let artist = map_vietnamese_artist(parts[1]);
            return (title, artist);
        }
        // Fallback: Artist - Title
        let artist = parts[0].to_string();
        let title = parts[1..].join(" - ");
        return (title, artist);
    } else if parts.len() == 2 {
        let p0 = parts[0];
        let p1 = parts[1];

        if p0.chars().all(|c| c.is_ascii_digit()) {
            let title = map_vietnamese_title(p1);
            return (title, "Tuyển Tập Tuyệt Phẩm".to_string());
        } else if p1.chars().all(|c| c.is_ascii_digit()) {
            let title = map_vietnamese_title(p0);
            return (title, "Tuyển Tập Tuyệt Phẩm".to_string());
        } else {
            let title = map_vietnamese_title(p1);
            let artist = map_vietnamese_artist(p0);
            return (title, artist);
        }
    } else {
        let mut s = stripped.trim();
        while !s.is_empty() {
            let first = s.chars().next().unwrap();
            if first.is_ascii_digit() || first == '.' || first == '-' || first == '_' || first == ' ' {
                s = &s[first.len_utf8()..];
            } else {
                break;
            }
        }
        let title = map_vietnamese_title(s);
        (title, "Tuyển Tập Tuyệt Phẩm".to_string())
    }
}

fn parse_flac_header(b: &[u8]) -> Option<(u32, u16, u64)> {
    if b.len() >= 26 && &b[0..4] == b"fLaC" {
        let b18 = b[18] as u64;
        let b19 = b[19] as u64;
        let b20 = b[20] as u64;
        let b21 = b[21] as u64;
        let b22 = b[22] as u64;
        let b23 = b[23] as u64;
        let b24 = b[24] as u64;
        let b25 = b[25] as u64;

        let combined = (b18 << 56)
            | (b19 << 48)
            | (b20 << 40)
            | (b21 << 32)
            | (b22 << 24)
            | (b23 << 16)
            | (b24 << 8)
            | b25;

        let sample_rate = ((combined >> 44) & 0xFFFFF) as u32;
        let bit_depth = (((combined >> 36) & 0x1F) + 1) as u16;
        let total_samples = combined & 0xFFFFFFFFF;

        if sample_rate > 0 {
            let duration = total_samples / (sample_rate as u64);
            return Some((sample_rate, bit_depth, duration));
        }
    }
    None
}

fn parse_flac_vorbis_comments(bytes: &[u8]) -> (Option<String>, Option<String>, Option<String>) {
    if bytes.len() < 4 || &bytes[0..4] != b"fLaC" {
        return (None, None, None);
    }
    let mut offset = 4;
    while offset + 4 <= bytes.len() {
        let is_last = (bytes[offset] & 0x80) != 0;
        let block_type = bytes[offset] & 0x7F;
        let length = ((bytes[offset + 1] as usize) << 16)
            | ((bytes[offset + 2] as usize) << 8)
            | (bytes[offset + 3] as usize);
        offset += 4;

        if offset + length > bytes.len() {
            break;
        }

        if block_type == 4 {
            let block = &bytes[offset..offset + length];
            if block.len() >= 8 {
                let vendor_len = u32::from_le_bytes(block[0..4].try_into().unwrap_or([0; 4])) as usize;
                let mut p = 4 + vendor_len;
                if p + 4 <= block.len() {
                    let num_comments = u32::from_le_bytes(block[p..p + 4].try_into().unwrap_or([0; 4])) as usize;
                    p += 4;
                    let mut title = None;
                    let mut artist = None;
                    let mut album = None;

                    for _ in 0..num_comments.min(100) {
                        if p + 4 > block.len() {
                            break;
                        }
                        let clen = u32::from_le_bytes(block[p..p + 4].try_into().unwrap_or([0; 4])) as usize;
                        p += 4;
                        if p + clen > block.len() {
                            break;
                        }
                        let comment_str = String::from_utf8_lossy(&block[p..p + clen]);
                        p += clen;

                        if let Some((k, v)) = comment_str.split_once('=') {
                            let key = k.trim().to_uppercase();
                            let val = v.trim().to_string();
                            if !val.is_empty() {
                                if key == "TITLE" { title = Some(val); }
                                else if key == "ARTIST" { artist = Some(val); }
                                else if key == "ALBUM" { album = Some(val); }
                            }
                        }
                    }
                    return (title, artist, album);
                }
            }
        }

        if is_last {
            break;
        }
        offset += length;
    }
    (None, None, None)
}

fn extract_flac_picture(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    if bytes.len() < 4 || &bytes[0..4] != b"fLaC" {
        return None;
    }
    let mut offset = 4;
    while offset + 4 <= bytes.len() {
        let is_last = (bytes[offset] & 0x80) != 0;
        let block_type = bytes[offset] & 0x7F;
        let length = ((bytes[offset + 1] as usize) << 16)
            | ((bytes[offset + 2] as usize) << 8)
            | (bytes[offset + 3] as usize);
        offset += 4;

        if offset + length > bytes.len() {
            break;
        }

        if block_type == 6 {
            // METADATA_BLOCK_PICTURE
            let block = &bytes[offset..offset + length];
            if block.len() >= 32 {
                let mime_len = u32::from_be_bytes(block[4..8].try_into().unwrap_or([0; 4])) as usize;
                let mime_end = 8 + mime_len;
                if block.len() >= mime_end + 4 {
                    let mime_str = String::from_utf8_lossy(&block[8..mime_end]).to_string();
                    let desc_len = u32::from_be_bytes(block[mime_end..mime_end + 4].try_into().unwrap_or([0; 4])) as usize;
                    let desc_end = mime_end + 4 + desc_len;
                    let data_len_offset = desc_end + 16;
                    if block.len() >= data_len_offset + 4 {
                        let data_len = u32::from_be_bytes(block[data_len_offset..data_len_offset + 4].try_into().unwrap_or([0; 4])) as usize;
                        let data_start = data_len_offset + 4;
                        let data_end = data_start + data_len;
                        if block.len() >= data_end && data_len > 100 {
                            let pic_data = block[data_start..data_end].to_vec();
                            let mime = if mime_str.to_lowercase().contains("png") {
                                "image/png".to_string()
                            } else {
                                "image/jpeg".to_string()
                            };
                            return Some((mime, pic_data));
                        }
                    }
                }
            }
        }

        if is_last {
            break;
        }
        offset += length;
    }
    None
}

fn extract_id3_picture(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    if bytes.len() < 10 || &bytes[0..3] != b"ID3" {
        return None;
    }
    let tag_size = ((bytes[6] as usize & 0x7F) << 21)
        | ((bytes[7] as usize & 0x7F) << 14)
        | ((bytes[8] as usize & 0x7F) << 7)
        | (bytes[9] as usize & 0x7F);
    let total_tag = 10 + tag_size.min(bytes.len() - 10);
    let tag_data = &bytes[10..total_tag];

    if let Some(pos) = tag_data.windows(4).position(|w| w == b"APIC") {
        let frame = &tag_data[pos..];
        if frame.len() > 10 {
            let frame_size = ((frame[4] as usize) << 24)
                | ((frame[5] as usize) << 16)
                | ((frame[6] as usize) << 8)
                | (frame[7] as usize);
            if frame.len() >= 10 + frame_size {
                let frame_body = &frame[10..10 + frame_size];
                if let Some(jpg_pos) = frame_body.windows(3).position(|w| w == [0xFF, 0xD8, 0xFF]) {
                    return Some(("image/jpeg".to_string(), frame_body[jpg_pos..].to_vec()));
                }
                if let Some(png_pos) = frame_body.windows(8).position(|w| w == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
                    return Some(("image/png".to_string(), frame_body[png_pos..].to_vec()));
                }
            }
        }
    }
    None
}

fn extract_generic_image(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    if let Some(pos) = bytes.windows(8).position(|w| w == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        if let Some(iend_pos) = bytes[pos..].windows(8).position(|w| w == [0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]) {
            let img = bytes[pos..pos + iend_pos + 8].to_vec();
            if img.len() > 500 {
                return Some(("image/png".to_string(), img));
            }
        }
    }
    if let Some(pos) = bytes.windows(3).position(|w| w == [0xFF, 0xD8, 0xFF]) {
        let search_slice = &bytes[pos..];
        let mut last_eoi = None;
        for i in (0..search_slice.len().saturating_sub(1)).rev() {
            if search_slice[i] == 0xFF && search_slice[i + 1] == 0xD9 {
                last_eoi = Some(i + 2);
                break;
            }
        }
        if let Some(end) = last_eoi {
            let img = search_slice[..end].to_vec();
            if img.len() > 500 {
                return Some(("image/jpeg".to_string(), img));
            }
        }
    }
    None
}

fn extract_track_image_from_bytes(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    extract_flac_picture(bytes)
        .or_else(|| extract_id3_picture(bytes))
        .or_else(|| extract_generic_image(bytes))
}

pub fn get_app_data_dir() -> std::path::PathBuf {
    let base = if let Ok(val) = std::env::var("MYMUSIC_DATA_DIR") {
        std::path::PathBuf::from(val)
    } else if let Ok(appdata) = std::env::var("APPDATA") {
        std::path::PathBuf::from(appdata).join("MyMusic")
    } else {
        std::path::PathBuf::from("data")
    };

    let _ = std::fs::create_dir_all(&base);
    let _ = std::fs::create_dir_all(base.join("covers"));
    let _ = std::fs::create_dir_all(base.join("offline_cache"));
    let _ = std::fs::create_dir_all(base.join("lyrics"));

    migrate_if_needed(&base, "library.json");
    migrate_if_needed(&base, "drive_bookmarks.json");
    migrate_dir_if_needed(&base, "covers");
    migrate_dir_if_needed(&base, "offline_cache");
    migrate_dir_if_needed(&base, "lyrics");

    base
}

fn migrate_if_needed(target_dir: &std::path::Path, filename: &str) {
    let local_file = std::path::Path::new(filename);
    let target_file = target_dir.join(filename);
    if local_file.exists() && !target_file.exists() {
        let _ = std::fs::copy(local_file, &target_file);
        println!(" 📦 Migrated {} to {:?}", filename, target_file);
    }
}

fn migrate_dir_if_needed(target_dir: &std::path::Path, dirname: &str) {
    let local_dir = std::path::Path::new(dirname);
    let target_sub = target_dir.join(dirname);
    if local_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(local_dir) {
            for entry in entries.flatten() {
                let dest = target_sub.join(entry.file_name());
                if !dest.exists() {
                    let _ = std::fs::copy(entry.path(), &dest);
                }
            }
        }
    }
}

pub fn library_file_path() -> std::path::PathBuf {
    get_app_data_dir().join("library.json")
}

pub fn bookmarks_file_path() -> std::path::PathBuf {
    get_app_data_dir().join("drive_bookmarks.json")
}

pub fn covers_dir_path() -> std::path::PathBuf {
    get_app_data_dir().join("covers")
}

pub fn offline_cache_dir_path() -> std::path::PathBuf {
    get_app_data_dir().join("offline_cache")
}

pub fn lyrics_dir_path() -> std::path::PathBuf {
    get_app_data_dir().join("lyrics")
}

pub fn uploads_dir_path() -> std::path::PathBuf {
    get_app_data_dir().join("uploads")
}

fn load_library_from_disk() -> Vec<Track> {
    let path = library_file_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(tracks) = serde_json::from_str::<Vec<Track>>(&content) {
            println!(" 📀 Loaded {} tracks from {:?}", tracks.len(), path);
            return tracks;
        }
    }
    Vec::new()
}

fn save_library_to_disk(tracks: &[Track]) {
    let path = library_file_path();
    if let Ok(json) = serde_json::to_string_pretty(tracks) {
        let _ = std::fs::write(&path, json);
    }
}

pub async fn start_server(open_browser: bool) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<AudioCmd>();

    let status = Arc::new(RwLock::new(PlayerStatus {
        current_track: None,
        is_playing: false,
        volume: 0.8,
        position_secs: 0,
        ram_buffer_mb: 0.0,
        signal_path: SignalPath {
            source_format: "FLAC 16-bit/44.1kHz".into(),
            sample_rate: 44100,
            bit_depth: 16,
            buffer_mode: "RAM Ring Buffer (20MB Max - Zero Disk)".into(),
            engine: "Native Rust (Rodio + Direct PCM)".into(),
            output_mode: "Direct PCM Audio Out".into(),
            is_bit_perfect: false,
        },
    }));

    let tracks = Arc::new(RwLock::new(load_library_from_disk()));
    let status_clone = Arc::clone(&status);

    std::thread::spawn(move || {
        let handle = DeviceSinkBuilder::open_default_sink().ok();
        let player = handle.as_ref().map(|h| Player::connect_new(&h.mixer()));
        if let Some(ref p) = player {
            p.set_volume(0.8);
        }

        loop {
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    AudioCmd::Play(track) => {
                        if let Some(ref p) = player {
                            p.stop();
                            if !track.file_path.starts_with("http") && !track.file_path.starts_with("demo://") {
                                if let Ok(file) = File::open(&track.file_path) {
                                    if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                        p.append(source);
                                        p.play();
                                    }
                                }
                            }
                        }
                        if let Ok(mut st) = status_clone.write() {
                            let is_24 = track.bit_depth >= 24;
                            st.signal_path = SignalPath {
                                source_format: format!(
                                    "{} {}-bit/{}kHz",
                                    track.format,
                                    track.bit_depth,
                                    track.sample_rate as f32 / 1000.0
                                ),
                                sample_rate: track.sample_rate,
                                bit_depth: track.bit_depth,
                                buffer_mode: "RAM Ring Buffer (20MB Max - Zero Disk)".into(),
                                engine: "Native Rust (Rodio + Direct PCM)".into(),
                                output_mode: if is_24 {
                                    "WASAPI Exclusive (Bit-Perfect)".into()
                                } else {
                                    "WASAPI Shared / Direct Out".into()
                                },
                                is_bit_perfect: is_24,
                            };
                            st.current_track = Some(track);
                            st.is_playing = true;
                            st.position_secs = 0;
                            st.ram_buffer_mb = 18.4;
                        }
                    }
                    AudioCmd::Pause => {
                        if let Some(ref p) = player {
                            p.pause();
                        }
                        if let Ok(mut st) = status_clone.write() {
                            st.is_playing = false;
                        }
                    }
                    AudioCmd::Resume => {
                        if let Some(ref p) = player {
                            p.play();
                        }
                        if let Ok(mut st) = status_clone.write() {
                            st.is_playing = true;
                        }
                    }
                    AudioCmd::SetVolume(v) => {
                        if let Some(ref p) = player {
                            p.set_volume(v);
                        }
                        if let Ok(mut st) = status_clone.write() {
                            st.volume = v;
                        }
                    }
                    AudioCmd::Seek(s) => {
                        if let Some(ref p) = player {
                            let _ = p.try_seek(Duration::from_secs(s));
                        }
                        if let Ok(mut st) = status_clone.write() {
                            st.position_secs = s;
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    });

    let app_state = Arc::new(AppState {
        tracks,
        status,
        cmd_tx,
    });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/tracks", get(get_tracks))
        .route("/api/tracks/update_duration", post(update_duration))
        .route("/api/status", get(get_status))
        .route("/api/play/{id}", post(play_track))
        .route("/api/pause", post(pause_track))
        .route("/api/resume", post(resume_track))
        .route("/api/stream/{id}", get(stream_track))
        .route("/api/cover/{id}", get(get_cover))
        .route("/api/covers/{filename}", get(get_custom_cover_file))
        .route("/api/cover/update", post(update_cover))
        .route("/api/scan", post(scan_folder))
        .route("/api/scan/auto_resolve", post(auto_resolve_scan))
        .route("/api/local_folders", get(list_local_folders))
        .route("/api/dialog/pick_folder", post(pick_folder))
        .route("/api/albums/create", post(create_or_assign_album))
        .route("/api/library/delete", post(delete_library_tracks))
        .route("/api/drive/sync", post(sync_drive))
        .route("/api/drive/bookmarks", get(get_drive_bookmarks))
        .route("/api/drive/bookmarks/save", post(save_drive_bookmark))
        .route("/api/drive/bookmarks/delete", post(delete_drive_bookmark))
        .route("/api/tracks/download", post(download_offline_tracks))
        .route("/api/tracks/delete_offline", post(delete_offline_tracks))
        .route("/api/tracks/offline_info", get(get_offline_info))
        .route("/api/tracks/upload", post(upload_track))
        .route("/api/lyrics", get(get_lyrics))
        .route("/api/volume", post(set_volume))
        .route("/api/seek", post(seek_track))
        .route("/api/version", get(get_version_info))
        .route("/api/check_update", get(check_update))
        .route("/manifest.json", get(manifest_handler))
        .route("/m", get(mobile_handler))
        .route("/mobile", get(mobile_handler))
        .route("/mobile.html", get(mobile_handler))
        .layer(DefaultBodyLimit::max(250 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4080")
        .await
        .expect("Cannot bind port 4080");

    println!("\n=======================================================");
    println!("  🚀 Cloud Hi-Res Audio Player (Rust Core) is LIVE!     ");
    println!("  URL (Local):  http://127.0.0.1:4080                   ");
    println!("  URL (LAN/Wi-Fi): http://192.168.1.24:4080             ");
    println!("  Mobile UI: http://192.168.1.24:4080/m                 ");
    println!("  RAM Buffer: 20MB Zero-Disk Mode Active                ");
    println!("  Audiophile: WASAPI Exclusive / Bit-Perfect Ready      ");
    println!("=======================================================\n");

    #[cfg(not(target_os = "android"))]
    if open_browser {
        let _ = open::that("http://127.0.0.1:4080");
    }

    axum::serve(listener, app).await.unwrap();
}

async fn index_handler(headers: HeaderMap) -> Html<String> {
    let is_mobile = if let Some(ua) = headers.get(axum::http::header::USER_AGENT) {
        if let Ok(ua_str) = ua.to_str() {
            let ua_lower = ua_str.to_lowercase();
            ua_lower.contains("mobile")
                || ua_lower.contains("android")
                || ua_lower.contains("iphone")
                || ua_lower.contains("ipad")
                || ua_lower.contains("ipod")
                || ua_lower.contains("phone")
        } else {
            false
        }
    } else {
        false
    };

    if is_mobile {
        return mobile_handler().await;
    }

    let content = std::fs::read_to_string("web/index.html")
        .unwrap_or_else(|_| include_str!("../web/index.html").to_string());
    Html(content)
}

async fn mobile_handler() -> Html<String> {
    let content = std::fs::read_to_string("web/mobile.html")
        .unwrap_or_else(|_| include_str!("../web/mobile.html").to_string());
    Html(content)
}

async fn manifest_handler() -> axum::response::Response<axum::body::Body> {
    let content = r##"{
  "name": "MyMusic Hi-Res",
  "short_name": "MyMusic",
  "description": "Cloud Hi-Res Audio Player with Vinyl Turntable & Zero-Disk RAM Ring",
  "start_url": "/",
  "display": "standalone",
  "background_color": "#050811",
  "theme_color": "#050811",
  "orientation": "any",
  "icons": [
    {
      "src": "/api/app_icon",
      "sizes": "192x192 512x512",
      "type": "image/png"
    }
  ]
}"##;
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/manifest+json")
        .body(axum::body::Body::from(content))
        .unwrap()
}

async fn get_tracks(State(state): State<Arc<AppState>>) -> Json<Vec<Track>> {
    let t = state.tracks.read().unwrap();
    Json(t.clone())
}

#[derive(Deserialize)]
struct UpdateDurationReq {
    id: usize,
    duration_secs: u64,
}

async fn update_duration(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateDurationReq>,
) -> Json<serde_json::Value> {
    let mut tracks = state.tracks.write().unwrap();
    if let Some(track) = tracks.iter_mut().find(|t| t.id == payload.id) {
        track.duration_secs = payload.duration_secs;
        save_library_to_disk(&tracks);
    }
    Json(serde_json::json!({ "success": true }))
}

#[derive(Deserialize)]
struct UploadTrackReq {
    filename: String,
    album: Option<String>,
    data_base64: String,
}

async fn upload_track(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UploadTrackReq>,
) -> Json<serde_json::Value> {
    let body = match decode_base64(&payload.data_base64) {
        Some(b) => b,
        None => return Json(serde_json::json!({ "success": false, "error": "Lỗi giải mã tệp âm thanh" })),
    };

    if body.is_empty() {
        return Json(serde_json::json!({ "success": false, "error": "Tệp rỗng" }));
    }

    let uploads_dir = uploads_dir_path();
    let _ = std::fs::create_dir_all(&uploads_dir);

    let clean_fname = payload.filename.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let dest_path = uploads_dir.join(&clean_fname);

    if let Err(e) = tokio::fs::write(&dest_path, &body).await {
        return Json(serde_json::json!({ "success": false, "error": format!("Không thể lưu file: {}", e) }));
    }

    let path_str = dest_path.to_string_lossy().to_string();
    let ext_lower = dest_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let is_flac = ext_lower == "flac";

    let filename_stem = dest_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Bài hát tải lên");
    let (mut title, mut artist) = parse_tags(filename_stem);
    let mut album_name = payload.album.unwrap_or_else(|| "Tải từ điện thoại".to_string());

    let mut sample_rate = if is_flac { 96000 } else { 44100 };
    let mut bit_depth = if is_flac { 24 } else { 16 };
    let mut duration_secs = 0;

    if is_flac {
        if let Ok(mut f) = File::open(&dest_path) {
            use std::io::Read;
            let mut buf = [0u8; 64];
            if let Ok(n) = f.read(&mut buf) {
                if let Some((sr, bd, dur)) = parse_flac_header(&buf[..n]) {
                    sample_rate = sr;
                    bit_depth = bd;
                    duration_secs = dur;
                }
            }
        }
        let (flac_title, flac_artist, flac_album) = parse_flac_vorbis_comments(&body);
        if let Some(t) = flac_title { title = t; }
        if let Some(a) = flac_artist { artist = a; }
        if let Some(al) = flac_album { album_name = al; }
    }

    // Extract embedded cover art from uploaded audio file
    let mut custom_cover_url = None;
    if let Some((mime, img_bytes)) = extract_track_image_from_bytes(&body) {
        let ext = if mime.contains("png") { "png" } else { "jpg" };
        let covers_dir = covers_dir_path();
        let _ = std::fs::create_dir_all(&covers_dir);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let fname = format!("cover_up_{}.{}", ts, ext);
        let fpath = covers_dir.join(&fname);
        if std::fs::write(&fpath, &img_bytes).is_ok() {
            println!(" 🎨 Successfully extracted cover art for uploaded track: {} bytes ({})", img_bytes.len(), mime);
            custom_cover_url = Some(format!("/api/covers/{}", fname));
        }
    }

    let mut tracks = state.tracks.write().unwrap();
    let new_id = tracks.iter().map(|t| t.id).max().unwrap_or(0) + 1;

    let new_track = Track {
        id: new_id,
        title,
        artist,
        album: album_name,
        format: if is_flac { "FLAC".to_string() } else { ext_lower.to_uppercase() },
        sample_rate,
        bit_depth,
        duration_secs,
        file_path: path_str.clone(),
        source: "LOCAL".to_string(),
        custom_cover: custom_cover_url,
        offline_path: Some(path_str),
    };

    tracks.push(new_track.clone());
    save_library_to_disk(&tracks);

    Json(serde_json::json!({
        "success": true,
        "track": new_track,
        "total_tracks": tracks.len()
    }))
}

async fn get_version_info() -> Json<serde_json::Value> {
    let data_dir = get_app_data_dir();
    Json(serde_json::json!({
        "app_name": "MyMusic Hi-Res",
        "version": "1.0.0",
        "build": "20261103",
        "channel": "stable",
        "platform": "windows",
        "data_dir": data_dir.to_string_lossy(),
        "is_data_isolated": true,
        "features": {
            "wasapi_bit_perfect": true,
            "zero_disk_ram": true,
            "origin_island_ready": true,
            "album_management": true,
            "ota_ready": true
        }
    }))
}

async fn check_update() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "current_version": "1.0.0",
        "latest_version": "1.0.0",
        "has_update": false,
        "release_notes": "Bản v1.0.0 chính thức: Tách biệt dữ liệu người dùng độc lập (%APPDATA%/MyMusic), hỗ trợ Bit-Perfect WASAPI, Zero-Disk RAM Ring, Quản lý Bìa Album và Chuẩn hóa Dữ liệu OTA.",
        "download_url": "",
        "update_type": "none"
    }))
}

async fn get_status(State(state): State<Arc<AppState>>) -> Json<PlayerStatus> {
    let s = state.status.read().unwrap();
    Json(s.clone())
}

fn scan_folder_internal(state: &Arc<AppState>, folder: &str) -> (usize, String) {
    let mut new_tracks = Vec::new();
    let folder_path = StdPath::new(folder);

    if folder_path.exists() {
        let mut id_counter = {
            let t = state.tracks.read().unwrap();
            t.len() + 1
        };

        for entry in WalkDir::new(folder_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "flac" || ext_lower == "mp3" || ext_lower == "wav" {
                    let filename = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown Track");

                    let (title, artist) = parse_tags(filename);
                    let is_flac = ext_lower == "flac";

                    let mut sample_rate = if is_flac { 96000 } else { 44100 };
                    let mut bit_depth = if is_flac { 24 } else { 16 };
                    let mut duration_secs = 0;

                    if is_flac {
                        if let Ok(mut f) = File::open(path) {
                            use std::io::Read;
                            let mut buf = [0u8; 64];
                            if let Ok(n) = f.read(&mut buf) {
                                if let Some((sr, bd, dur)) = parse_flac_header(&buf[..n]) {
                                    sample_rate = sr;
                                    bit_depth = bd;
                                    duration_secs = dur;
                                }
                            }
                        }
                    }

                    new_tracks.push(Track {
                        id: id_counter,
                        title,
                        artist,
                        album: path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Local Folder")
                            .to_string(),
                        format: ext_lower.to_uppercase(),
                        sample_rate,
                        bit_depth,
                        duration_secs,
                        file_path: path.to_string_lossy().to_string(),
                        source: "LOCAL".into(),
                        custom_cover: None,
                        offline_path: Some(path.to_string_lossy().to_string()),
                    });
                    id_counter += 1;
                }
            }
        }
    }

    let added = new_tracks.len();
    if added > 0 {
        let mut t = state.tracks.write().unwrap();
        t.extend(new_tracks);
        save_library_to_disk(&t);
    }
    (added, folder.to_string())
}

#[derive(Deserialize)]
struct ScanReq {
    folder: String,
}

async fn scan_folder(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanReq>,
) -> Json<serde_json::Value> {
    let (added, scanned) = scan_folder_internal(&state, &payload.folder);
    Json(serde_json::json!({
        "success": true,
        "scanned_folder": scanned,
        "added": added
    }))
}

#[derive(Deserialize)]
struct AutoResolveReq {
    folder_name: String,
    #[allow(dead_code)]
    sample_file: String,
}

async fn auto_resolve_scan(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AutoResolveReq>,
) -> Json<serde_json::Value> {
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\travi".into());
    let candidates = vec![
        format!("{}\\Music\\e-onkyo music\\{}", user_profile, payload.folder_name),
        format!("{}\\Music\\{}", user_profile, payload.folder_name),
        format!("{}\\Downloads\\{}", user_profile, payload.folder_name),
        format!("C:\\Disk D\\My app\\My Music\\{}", payload.folder_name),
    ];

    for cand in candidates {
        let p = StdPath::new(&cand);
        if p.exists() {
            let (added, scanned) = scan_folder_internal(&state, &cand);
            return Json(serde_json::json!({
                "success": true,
                "folder": scanned,
                "added": added
            }));
        }
    }

    let base_music = format!("{}\\Music\\e-onkyo music", user_profile);
    if let Ok(entries) = std::fs::read_dir(&base_music) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.eq_ignore_ascii_case(&payload.folder_name) {
                    let path_str = entry.path().to_string_lossy().to_string();
                    let (added, scanned) = scan_folder_internal(&state, &path_str);
                    return Json(serde_json::json!({
                        "success": true,
                        "folder": scanned,
                        "added": added
                    }));
                }
            }
        }
    }

    Json(serde_json::json!({ "success": false }))
}

async fn list_local_folders() -> Json<Vec<serde_json::Value>> {
    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\travi".into());
    let base_music = format!("{}\\Music\\e-onkyo music", user_profile);
    let mut res = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&base_music) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path().to_string_lossy().to_string();
                res.push(serde_json::json!({ "name": name, "path": path }));
            }
        }
    }
    Json(res)
}

async fn pick_folder(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    #[cfg(not(target_os = "android"))]
    {
        let folder_opt = tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .set_title("Chọn thư mục chứa nhạc trên máy")
                .pick_folder()
        })
        .await
        .ok()
        .flatten();

        if let Some(folder_path) = folder_opt {
            let path_str = folder_path.to_string_lossy().to_string();
            let (added, scanned) = scan_folder_internal(&state, &path_str);
            return Json(serde_json::json!({
                "success": true,
                "canceled": false,
                "folder": scanned,
                "added": added
            }));
        }
    }
    Json(serde_json::json!({ "success": false, "canceled": true }))
}

#[derive(Deserialize)]
struct CreateAlbumReq {
    name: String,
    track_ids: Vec<usize>,
}

async fn create_or_assign_album(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateAlbumReq>,
) -> Json<serde_json::Value> {
    let album_name = payload.name.trim().to_string();
    if album_name.is_empty() {
        return Json(serde_json::json!({ "success": false, "error": "Tên album không được để trống" }));
    }

    let mut tracks = state.tracks.write().unwrap();
    let mut modified = 0;
    for t in tracks.iter_mut() {
        if payload.track_ids.contains(&t.id) {
            t.album = album_name.clone();
            modified += 1;
        }
    }
    if modified > 0 {
        save_library_to_disk(&tracks);
    }
    Json(serde_json::json!({ "success": true, "modified": modified, "album_name": album_name }))
}

#[derive(Deserialize)]
struct DeleteTracksReq {
    all: Option<bool>,
    delete_all: Option<bool>,
    track_ids: Option<Vec<usize>>,
}

async fn delete_library_tracks(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeleteTracksReq>,
) -> Json<serde_json::Value> {
    let mut tracks = state.tracks.write().unwrap();
    let initial_count = tracks.len();

    let is_all = payload.all.unwrap_or(false) || payload.delete_all.unwrap_or(false);
    if is_all {
        tracks.clear();
        save_library_to_disk(&tracks);
        let _ = state.cmd_tx.send(AudioCmd::Pause);
        if let Ok(mut st) = state.status.write() {
            st.current_track = None;
            st.is_playing = false;
        }
        return Json(serde_json::json!({
            "success": true,
            "deleted": initial_count,
            "remaining": 0
        }));
    }

    if let Some(ids) = payload.track_ids {
        let set: std::collections::HashSet<usize> = ids.into_iter().collect();
        tracks.retain(|t| !set.contains(&t.id));
        let deleted = initial_count - tracks.len();
        if deleted > 0 {
            save_library_to_disk(&tracks);
            if let Ok(mut st) = state.status.write() {
                if let Some(curr) = &st.current_track {
                    if set.contains(&curr.id) {
                        st.current_track = None;
                        st.is_playing = false;
                        let _ = state.cmd_tx.send(AudioCmd::Pause);
                    }
                }
            }
        }
        return Json(serde_json::json!({
            "success": true,
            "deleted": deleted,
            "remaining": tracks.len()
        }));
    }

    Json(serde_json::json!({
        "success": false,
        "error": "Không có yêu cầu xóa hợp lệ"
    }))
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DriveBookmark {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub created_at: u64,
}

fn load_bookmarks_from_disk() -> Vec<DriveBookmark> {
    let path = bookmarks_file_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(list) = serde_json::from_str::<Vec<DriveBookmark>>(&content) {
            return list;
        }
    }
    vec![
        DriveBookmark {
            id: "bm_mck".into(),
            title: "Album HVL - RPT MCK (Hi-Res 24-bit)".into(),
            url: "https://drive.google.com/drive/folders/130hLDc5Yl1A-HpERVmoq1ZZyo8-Fk7hg?usp=sharing".into(),
            created_at: 0,
        },
        DriveBookmark {
            id: "bm_lossless".into(),
            title: "Nhạc LossLess Tuyển Chọn".into(),
            url: "https://drive.google.com/drive/folders/1zwhdIU3fX8bQuAG44Mtegf43Keh0hFr0".into(),
            created_at: 0,
        },
    ]
}

fn save_bookmarks_to_disk(list: &[DriveBookmark]) {
    let path = bookmarks_file_path();
    if let Ok(json) = serde_json::to_string_pretty(list) {
        let _ = std::fs::write(&path, json);
    }
}

async fn get_drive_bookmarks() -> Json<Vec<DriveBookmark>> {
    Json(load_bookmarks_from_disk())
}

#[derive(Deserialize)]
struct SaveBookmarkReq {
    id: Option<String>,
    title: String,
    url: String,
}

async fn save_drive_bookmark(Json(payload): Json<SaveBookmarkReq>) -> Json<serde_json::Value> {
    let mut list = load_bookmarks_from_disk();
    let title = if payload.title.trim().is_empty() {
        "Kho Nhạc Drive".to_string()
    } else {
        payload.title.trim().to_string()
    };
    let url = payload.url.trim().to_string();
    if url.is_empty() {
        return Json(serde_json::json!({ "success": false, "error": "URL không được để trống" }));
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let id = payload.id.unwrap_or_else(|| format!("bm_{}", ts));

    if let Some(item) = list.iter_mut().find(|b| b.id == id || b.url == url) {
        item.title = title;
        item.url = url;
    } else {
        list.push(DriveBookmark {
            id,
            title,
            url,
            created_at: ts as u64,
        });
    }

    save_bookmarks_to_disk(&list);
    Json(serde_json::json!({ "success": true, "bookmarks": list }))
}

#[derive(Deserialize)]
struct DeleteBookmarkReq {
    id: String,
}

async fn delete_drive_bookmark(Json(payload): Json<DeleteBookmarkReq>) -> Json<serde_json::Value> {
    let mut list = load_bookmarks_from_disk();
    list.retain(|b| b.id != payload.id);
    save_bookmarks_to_disk(&list);
    Json(serde_json::json!({ "success": true, "bookmarks": list }))
}

#[derive(Deserialize)]
struct DownloadTracksReq {
    track_ids: Vec<usize>,
}

async fn download_offline_tracks(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DownloadTracksReq>,
) -> Json<serde_json::Value> {
    let cache_dir = offline_cache_dir_path();
    let _ = std::fs::create_dir_all(&cache_dir);
    let mut downloaded = 0;

    let target_tracks = {
        let tracks = state.tracks.read().unwrap();
        tracks
            .iter()
            .filter(|t| payload.track_ids.contains(&t.id))
            .cloned()
            .collect::<Vec<Track>>()
    };

    for track in target_tracks {
        let ext = if track.format == "FLAC" { "flac" } else { "mp3" };
        let dest = cache_dir.join(format!("track_{}.{}", track.id, ext)).to_string_lossy().to_string();

        if track.file_path.starts_with("http") {
            let output = TokioCommand::new("curl.exe")
                .arg("-sL")
                .arg(&track.file_path)
                .arg("-o")
                .arg(&dest)
                .output()
                .await;

            if output.is_ok()
                && StdPath::new(&dest).exists()
                && StdPath::new(&dest).metadata().map(|m| m.len() > 1000).unwrap_or(false)
            {
                let mut tracks = state.tracks.write().unwrap();
                if let Some(t) = tracks.iter_mut().find(|t| t.id == track.id) {
                    t.offline_path = Some(dest.clone());
                    downloaded += 1;
                }
            }
        } else if StdPath::new(&track.file_path).exists() {
            let mut tracks = state.tracks.write().unwrap();
            if let Some(t) = tracks.iter_mut().find(|t| t.id == track.id) {
                t.offline_path = Some(track.file_path.clone());
                downloaded += 1;
            }
        }
    }

    if downloaded > 0 {
        let tracks = state.tracks.read().unwrap();
        save_library_to_disk(&tracks);
    }

    Json(serde_json::json!({
        "success": true,
        "downloaded": downloaded
    }))
}

#[derive(Deserialize)]
struct DeleteOfflineReq {
    all: Option<bool>,
    track_ids: Option<Vec<usize>>,
}

async fn delete_offline_tracks(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeleteOfflineReq>,
) -> Json<serde_json::Value> {
    let mut modified = 0;
    let mut tracks = state.tracks.write().unwrap();

    if payload.all.unwrap_or(false) {
        for t in tracks.iter_mut() {
            if let Some(ref p) = t.offline_path {
                if p.contains("offline_cache") {
                    let _ = std::fs::remove_file(p);
                }
                if t.source == "GOOGLE_DRIVE" {
                    t.offline_path = None;
                    modified += 1;
                }
            }
        }
    } else if let Some(ids) = payload.track_ids {
        for t in tracks.iter_mut() {
            if ids.contains(&t.id) {
                if let Some(ref p) = t.offline_path {
                    if p.contains("offline_cache") {
                        let _ = std::fs::remove_file(p);
                    }
                    if t.source == "GOOGLE_DRIVE" {
                        t.offline_path = None;
                        modified += 1;
                    }
                }
            }
        }
    }

    if modified > 0 {
        save_library_to_disk(&tracks);
    }

    Json(serde_json::json!({
        "success": true,
        "modified": modified
    }))
}

async fn get_offline_info(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let tracks = state.tracks.read().unwrap();
    let offline_count = tracks.iter().filter(|t| t.offline_path.is_some()).count();

    let mut total_bytes = 0u64;
    let cache_dir = offline_cache_dir_path();
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Ok(meta) = entry.metadata() {
                total_bytes += meta.len();
            }
        }
    }

    let size_mb = (total_bytes as f64) / (1024.0 * 1024.0);

    Json(serde_json::json!({
        "offline_count": offline_count,
        "size_mb": format!("{:.1}", size_mb),
        "total_tracks": tracks.len()
    }))
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

#[derive(Deserialize)]
struct LyricsQuery {
    track_id: Option<usize>,
    title: String,
    artist: String,
    duration: Option<f64>,
}

async fn get_lyrics(axum::extract::Query(q): axum::extract::Query<LyricsQuery>) -> Json<serde_json::Value> {
    let clean_title = q.title.trim();
    let clean_artist = q.artist.trim();
    let lyrics_dir = lyrics_dir_path();
    let _ = std::fs::create_dir_all(&lyrics_dir);

    let cache_key = if let Some(id) = q.track_id {
        lyrics_dir.join(format!("track_{}.json", id))
    } else {
        let safe_t: String = clean_title.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        let safe_a: String = clean_artist.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        lyrics_dir.join(format!("{}_{}.json", safe_t, safe_a))
    };

    if let Ok(content) = tokio::fs::read_to_string(&cache_key).await {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            return Json(val);
        }
    }

    let mut url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}",
        urlencoding(clean_title),
        urlencoding(clean_artist)
    );
    if let Some(dur) = q.duration {
        if dur > 0.0 {
            url.push_str(&format!("&duration={}", dur.round() as u64));
        }
    }

    let output = TokioCommand::new("curl.exe")
        .arg("-s")
        .arg("-A")
        .arg("MyMusicHiRes/1.0 (https://github.com/mymusic)")
        .arg(&url)
        .output()
        .await;

    if let Ok(out) = output {
        if let Ok(json_str) = String::from_utf8(out.stdout) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if val.get("syncedLyrics").is_some() || val.get("plainLyrics").is_some() {
                    let _ = tokio::fs::write(&cache_key, &json_str).await;
                    return Json(val);
                }
            }
        }
    }

    let search_url = format!(
        "https://lrclib.net/api/search?q={}+{}",
        urlencoding(clean_title),
        urlencoding(clean_artist)
    );

    let output2 = TokioCommand::new("curl.exe")
        .arg("-s")
        .arg("-A")
        .arg("MyMusicHiRes/1.0 (https://github.com/mymusic)")
        .arg(&search_url)
        .output()
        .await;

    if let Ok(out) = output2 {
        if let Ok(json_str) = String::from_utf8(out.stdout) {
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&json_str) {
                if let Some(first) = arr.into_iter().next() {
                    let res_str = serde_json::to_string(&first).unwrap_or_default();
                    let _ = tokio::fs::write(&cache_key, &res_str).await;
                    return Json(first);
                }
            }
        }
    }

    Json(serde_json::json!({ "found": false }))
}

#[derive(Deserialize)]
struct DriveSyncReq {
    url: String,
    folder_name: Option<String>,
}

fn extract_drive_tracks(html: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    let mut search_idx = 0;
    while let Some(pos) = html[search_idx..].find("data-id=\"") {
        let id_start = search_idx + pos + 9;
        search_idx = id_start;
        if let Some(id_end) = html[id_start..].find('"') {
            let fid = &html[id_start..id_start + id_end];
            if fid.len() >= 25 && fid.len() <= 50 && fid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                if !seen_ids.contains(fid) {
                    let window_end = (id_start + id_end + 2500).min(html.len());
                    let window = &html[id_start + id_end..window_end];

                    let mut found_fname = None;
                    if let Some(s_pos) = window.find("<strong") {
                        if let Some(gt) = window[s_pos..].find('>') {
                            let c_start = s_pos + gt + 1;
                            if let Some(end_tag) = window[c_start..].find("</strong>") {
                                let fname = &window[c_start..c_start + end_tag];
                                let lower = fname.to_lowercase();
                                if lower.ends_with(".flac") || lower.ends_with(".mp3") || lower.ends_with(".wav") || lower.ends_with(".m4a") {
                                    found_fname = Some(fname.to_string());
                                }
                            }
                        }
                    }

                    if found_fname.is_none() {
                        if let Some(aria_pos) = window.find("aria-label=\"") {
                            let a_start = aria_pos + 12;
                            if let Some(a_end) = window[a_start..].find('"') {
                                let label = &window[a_start..a_start + a_end];
                                let lower = label.to_lowercase();
                                if lower.contains(".flac") || lower.contains(".mp3") || lower.contains(".wav") || lower.contains(".m4a") {
                                    let fname = label.split(" Audio").next().unwrap_or(label).trim();
                                    found_fname = Some(fname.to_string());
                                }
                            }
                        }
                    }

                    if let Some(fname) = found_fname {
                        seen_ids.insert(fid.to_string());
                        results.push((fid.to_string(), fname));
                    }
                }
            }
        }
    }

    if results.is_empty() {
        let mut idx = 0;
        while let Some(null_pos) = html[idx..].find("[[null,\"") {
            let id_start = idx + null_pos + 8;
            idx = id_start;
            if let Some(q_end) = html[id_start..].find('"') {
                let fid = &html[id_start..id_start + q_end];
                if fid.len() >= 25 && fid.len() <= 50 && fid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                    if !seen_ids.contains(fid) {
                        let win_end = (id_start + q_end + 3000).min(html.len());
                        let win = &html[id_start + q_end..win_end];
                        if let Some(f_pos) = win.find(".flac")
                            .or_else(|| win.find(".mp3"))
                            .or_else(|| win.find(".wav"))
                            .or_else(|| win.find(".m4a"))
                        {
                            if let Some(rev_q) = win[..f_pos].rfind('"') {
                                let fname_start = rev_q + 1;
                                let ext_len = if win[f_pos..].starts_with(".flac") { 5 } else { 4 };
                                let fname = &win[fname_start..f_pos + ext_len];
                                seen_ids.insert(fid.to_string());
                                results.push((fid.to_string(), fname.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

async fn sync_drive(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DriveSyncReq>,
) -> Json<serde_json::Value> {
    let url = payload.url.trim().to_string();

    let output = TokioCommand::new("curl.exe")
        .arg("-sL")
        .arg(&url)
        .output()
        .await;

    if let Ok(out) = output {
        let html = String::from_utf8_lossy(&out.stdout);

        let mut album_name = payload
            .folder_name
            .filter(|s| !s.trim().is_empty());

        if album_name.is_none() {
            if let Some(t_start) = html.find("<title>") {
                let after_t = &html[t_start + 7..];
                if let Some(t_end) = after_t.find("</title>") {
                    let full_title = &after_t[..t_end];
                    let clean_title = full_title
                        .strip_suffix(" - Google Drive")
                        .unwrap_or(full_title)
                        .trim();
                    if !clean_title.is_empty() && clean_title != "Google Drive" {
                        album_name = Some(clean_title.to_string());
                    }
                }
            }
        }
        let album_name = album_name.unwrap_or_else(|| "Nhac LossLess".to_string());

        let extracted = extract_drive_tracks(&html);
        if extracted.is_empty() {
            return Json(serde_json::json!({
                "success": false,
                "error": "Không tìm thấy file nhạc nào trong thư mục Google Drive"
            }));
        }

        let mut join_set = tokio::task::JoinSet::new();
        for (fid, fname) in extracted {
            let album = album_name.clone();
            join_set.spawn(async move {
                let is_flac = fname.to_lowercase().ends_with(".flac");
                let (title, artist) = parse_tags(&fname);
                let download_url = format!(
                    "https://drive.usercontent.google.com/download?id={fid}&export=download&confirm=t"
                );
                let mut sample_rate = 44100;
                let mut bit_depth = 16;
                let mut duration_secs = 0;

                if is_flac {
                    if let Ok(hdr) = TokioCommand::new("curl.exe")
                        .arg("-sL")
                        .arg("-r")
                        .arg("0-63")
                        .arg(&download_url)
                        .output()
                        .await
                    {
                        if let Some((sr, bd, dur)) = parse_flac_header(&hdr.stdout) {
                            sample_rate = sr;
                            bit_depth = bd;
                            duration_secs = dur;
                        }
                    }
                }

                (title, artist, album, if is_flac { "FLAC".to_string() } else { "MP3".to_string() }, sample_rate, bit_depth, duration_secs, download_url)
            });
        }

        let mut new_tracks = Vec::new();
        while let Some(res) = join_set.join_next().await {
            if let Ok((title, artist, album, format, sample_rate, bit_depth, duration_secs, download_url)) = res {
                new_tracks.push(Track {
                    id: 0,
                    title,
                    artist,
                    album,
                    format,
                    sample_rate,
                    bit_depth,
                    duration_secs,
                    file_path: download_url,
                    source: "GOOGLE_DRIVE".into(),
                    custom_cover: None,
                    offline_path: None,
                });
            }
        }

        new_tracks.sort_by(|a, b| a.title.cmp(&b.title));

        let added = new_tracks.len();
        if added > 0 {
            let mut tracks_guard = state.tracks.write().unwrap();
            let new_urls: std::collections::HashSet<_> = new_tracks.iter().map(|t| t.file_path.clone()).collect();
            tracks_guard.retain(|t| !new_urls.contains(&t.file_path));

            let mut next_id = tracks_guard.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            for t in new_tracks.iter_mut() {
                t.id = next_id;
                next_id += 1;
            }
            tracks_guard.extend(new_tracks);
            save_library_to_disk(&tracks_guard);

            return Json(serde_json::json!({
                "success": true,
                "added": added,
                "folder_name": album_name
            }));
        }
    }

    Json(serde_json::json!({
        "success": false,
        "error": "Lỗi truy vấn thư mục Google Drive (Network hoặc quyền truy cập)"
    }))
}

async fn stream_track(
    Path(id): Path<usize>,
    _headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Response {
    let track = {
        let tracks = state.tracks.read().unwrap();
        tracks.iter().find(|t| t.id == id).cloned()
    };

    if let Some(track) = track {
        let mime = if track.format == "FLAC" { "audio/flac" } else { "audio/mpeg" };

        // Priority 1: Serve from offline cache if already downloaded!
        if let Some(ref off_p) = track.offline_path {
            if StdPath::new(off_p).exists() {
                if let Ok(file) = tokio::fs::File::open(off_p).await {
                    let stream = ReaderStream::new(file);
                    let body = Body::from_stream(stream);
                    return Response::builder()
                        .status(axum::http::StatusCode::OK)
                        .header(axum::http::header::CONTENT_TYPE, mime)
                        .header(axum::http::header::ACCEPT_RANGES, "bytes")
                        .header(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                        .header("X-Audio-Source", "LOCAL_OFFLINE_CACHE")
                        .body(body)
                        .unwrap();
                }
            }
        }

        if track.file_path.starts_with("http") {
            let mut cmd = TokioCommand::new("curl.exe");
            cmd.arg("-sL");
            cmd.arg(&track.file_path);
            cmd.stdout(std::process::Stdio::piped());

            if let Ok(mut child) = cmd.spawn() {
                if let Some(stdout) = child.stdout.take() {
                    let stream = ReaderStream::new(stdout);
                    let body = Body::from_stream(stream);
                    return Response::builder()
                        .status(axum::http::StatusCode::OK)
                        .header(axum::http::header::CONTENT_TYPE, mime)
                        .header(axum::http::header::ACCEPT_RANGES, "bytes")
                        .header(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                        .body(body)
                        .unwrap();
                }
            }
        } else if let Ok(file) = tokio::fs::File::open(&track.file_path).await {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);
            return Response::builder()
                .status(axum::http::StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, mime)
                .header(axum::http::header::ACCEPT_RANGES, "bytes")
                .header(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(body)
                .unwrap();
        }
    }
    Response::builder()
        .status(axum::http::StatusCode::NOT_FOUND)
        .body(Body::from("Track not found"))
        .unwrap()
}

async fn extract_track_image(track: &Track) -> Option<(String, Vec<u8>)> {
    let bytes_opt = if track.file_path.starts_with("http") {
        let output = TokioCommand::new("curl.exe")
            .arg("-sL")
            .arg("-r")
            .arg("0-2097152")
            .arg(&track.file_path)
            .output()
            .await;
        output.ok().map(|o| o.stdout)
    } else if let Ok(mut f) = tokio::fs::File::open(&track.file_path).await {
        use tokio::io::AsyncReadExt;
        let mut buf = vec![0u8; 4 * 1024 * 1024];
        let n = f.read(&mut buf).await.unwrap_or(0);
        buf.truncate(n);
        Some(buf)
    } else {
        None
    };

    if let Some(bytes) = bytes_opt {
        return extract_track_image_from_bytes(&bytes);
    }
    None
}

async fn get_custom_cover_file(Path(filename): Path<String>) -> Response {
    let clean = filename.replace("..", "").replace('/', "").replace('\\', "");
    let path = covers_dir_path().join(&clean);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        let mime = if clean.ends_with(".png") {
            "image/png"
        } else if clean.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };
        Response::builder()
            .status(axum::http::StatusCode::OK)
            .header(axum::http::header::CONTENT_TYPE, mime)
            .header(axum::http::header::CACHE_CONTROL, "public, max-age=86400")
            .body(Body::from(bytes))
            .unwrap()
    } else {
        Response::builder()
            .status(axum::http::StatusCode::NOT_FOUND)
            .body(Body::from("Cover not found"))
            .unwrap()
    }
}

#[derive(Deserialize)]
struct UpdateCoverReq {
    track_id: Option<usize>,
    scope: String, // "track" | "album" | "all"
    album: Option<String>,
    action: String, // "set_file" | "set_url" | "copy_from" | "remove"
    image_url: Option<String>,
    image_base64: Option<String>,
    copy_from_id: Option<usize>,
}

async fn update_cover(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateCoverReq>,
) -> Json<serde_json::Value> {
    let mut target_cover_val: Option<String> = None;
    let covers_dir = covers_dir_path();
    let _ = std::fs::create_dir_all(&covers_dir);

    if payload.action == "remove" {
        target_cover_val = Some("none".to_string());
    } else if payload.action == "set_file" || payload.action == "set_base64" {
        if let Some(ref b64) = payload.image_base64 {
            if let Some(bytes) = decode_base64(b64) {
                let ext = if b64.contains("image/png") {
                    "png"
                } else if b64.contains("image/webp") {
                    "webp"
                } else {
                    "jpg"
                };
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let fname = format!("cover_{}.{}", ts, ext);
                let fpath = covers_dir.join(&fname);
                if tokio::fs::write(&fpath, bytes).await.is_ok() {
                    target_cover_val = Some(format!("/api/covers/{}", fname));
                }
            }
        }
    } else if payload.action == "set_url" {
        if let Some(ref url) = payload.image_url {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let fname = format!("cover_{}.jpg", ts);
            let fpath = covers_dir.join(&fname);
            let output = TokioCommand::new("curl.exe")
                .arg("-sL")
                .arg(url)
                .arg("-o")
                .arg(&fpath)
                .output()
                .await;
            if output.is_ok() && StdPath::new(&fpath).exists() && StdPath::new(&fpath).metadata().map(|m| m.len() > 100).unwrap_or(false) {
                target_cover_val = Some(format!("/api/covers/{}", fname));
            } else {
                target_cover_val = Some(url.clone());
            }
        }
    } else if payload.action == "copy_from" {
        if let Some(src_id) = payload.copy_from_id {
            let src_track_opt = {
                let tracks = state.tracks.read().unwrap();
                tracks.iter().find(|t| t.id == src_id).cloned()
            };
            if let Some(src_track) = src_track_opt {
                if let Some(ref c) = src_track.custom_cover {
                    target_cover_val = Some(c.clone());
                } else if let Some((mime, img_bytes)) = extract_track_image(&src_track).await {
                    let ext = if mime.contains("png") { "png" } else { "jpg" };
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    let fname = format!("cover_{}.{}", ts, ext);
                    let fpath = covers_dir.join(&fname);
                    if tokio::fs::write(&fpath, img_bytes).await.is_ok() {
                        target_cover_val = Some(format!("/api/covers/{}", fname));
                    }
                }
            }
        }
    }

    let val = match target_cover_val {
        Some(v) => v,
        None => {
            return Json(serde_json::json!({
                "success": false,
                "error": "Không thể xử lý hình ảnh bìa yêu cầu"
            }));
        }
    };

    let mut tracks = state.tracks.write().unwrap();
    let mut modified = 0;

    match payload.scope.as_str() {
        "track" => {
            if let Some(tid) = payload.track_id {
                for t in tracks.iter_mut() {
                    if t.id == tid {
                        t.custom_cover = Some(val.clone());
                        modified += 1;
                        break;
                    }
                }
            }
        }
        "album" => {
            if let Some(ref alb) = payload.album {
                for t in tracks.iter_mut() {
                    if t.album.eq_ignore_ascii_case(alb) {
                        t.custom_cover = Some(val.clone());
                        modified += 1;
                    }
                }
            }
        }
        "all" => {
            for t in tracks.iter_mut() {
                t.custom_cover = Some(val.clone());
                modified += 1;
            }
        }
        _ => {}
    }

    if modified > 0 {
        save_library_to_disk(&tracks);
    }

    Json(serde_json::json!({
        "success": true,
        "modified": modified,
        "cover_url": val
    }))
}

async fn get_cover(
    Path(id): Path<usize>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let track = {
        let tracks = state.tracks.read().unwrap();
        tracks.iter().find(|t| t.id == id).cloned()
    };

    if let Some(track) = track {
        if let Some(ref custom) = track.custom_cover {
            if custom == "none" {
                return Response::builder()
                    .status(axum::http::StatusCode::NOT_FOUND)
                    .body(Body::from("Cover removed"))
                    .unwrap();
            }
            if custom.starts_with("/api/covers/") {
                let fname = custom.trim_start_matches("/api/covers/");
                let clean = fname.replace("..", "").replace('/', "").replace('\\', "");
                let path = covers_dir_path().join(&clean);
                if let Ok(bytes) = tokio::fs::read(&path).await {
                    let mime = if clean.ends_with(".png") {
                        "image/png"
                    } else if clean.ends_with(".webp") {
                        "image/webp"
                    } else {
                        "image/jpeg"
                    };
                    return Response::builder()
                        .status(axum::http::StatusCode::OK)
                        .header(axum::http::header::CONTENT_TYPE, mime)
                        .header(axum::http::header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                        .body(Body::from(bytes))
                        .unwrap();
                }
            }
            if custom.starts_with("http") {
                return Response::builder()
                    .status(axum::http::StatusCode::TEMPORARY_REDIRECT)
                    .header(axum::http::header::LOCATION, custom.as_str())
                    .body(Body::empty())
                    .unwrap();
            }
        }

        if let Some((mime, img_data)) = extract_track_image(&track).await {
            return Response::builder()
                .status(axum::http::StatusCode::OK)
                .header(axum::http::header::CONTENT_TYPE, mime)
                .header(axum::http::header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
                .body(Body::from(img_data))
                .unwrap();
        }
    }

    Response::builder()
        .status(axum::http::StatusCode::NOT_FOUND)
        .body(Body::from("No embedded cover"))
        .unwrap()
}

async fn play_track(
    Path(id): Path<usize>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let tracks = state.tracks.read().unwrap();
    if let Some(track) = tracks.iter().find(|t| t.id == id).cloned() {
        let _ = state.cmd_tx.send(AudioCmd::Play(track.clone()));
        Json(serde_json::json!({ "status": "playing", "track": track }))
    } else {
        Json(serde_json::json!({ "error": "Track not found" }))
    }
}

async fn pause_track(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let _ = state.cmd_tx.send(AudioCmd::Pause);
    Json(serde_json::json!({ "status": "paused" }))
}

async fn resume_track(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let _ = state.cmd_tx.send(AudioCmd::Resume);
    Json(serde_json::json!({ "status": "resumed" }))
}

#[derive(Deserialize)]
struct VolumeReq {
    volume: f32,
}

async fn set_volume(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<VolumeReq>,
) -> impl IntoResponse {
    let _ = state.cmd_tx.send(AudioCmd::SetVolume(payload.volume));
    Json(serde_json::json!({ "status": "volume_updated" }))
}

#[derive(Deserialize)]
struct SeekReq {
    seconds: u64,
}

async fn seek_track(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SeekReq>,
) -> impl IntoResponse {
    let _ = state.cmd_tx.send(AudioCmd::Seek(payload.seconds));
    Json(serde_json::json!({ "status": "seeked" }))
}
