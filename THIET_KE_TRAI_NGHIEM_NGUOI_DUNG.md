# TÀI LIỆU THIẾT KẾ TRẢI NGHIỆM NGƯỜI DÙNG & GIAO DIỆN (UI/UX)
**Dự án:** Cloud Hi-Res Audio Player (Android & Windows)  
**Nguồn cảm hứng:** Roon 2.0 (Desktop) & Roon ARC (Mobile)  
**Triết lý:** Tạp chí âm nhạc số (Editorial), Tối giản thanh lịch, Minh bạch chất lượng Audiophile (Signal Path).

---

## I. TRIẾT LÝ THIẾT KẾ TỔNG THỂ

1. **Từ "Trình phát file" thành "Không gian thưởng thức nghệ thuật":**
   * Xóa bỏ hoàn toàn định kiến app nghe nhạc từ Google Drive là "duyệt cây thư mục khô khan như Windows Explorer".
   * Giao diện tổ chức theo ngôn ngữ **Tạp chí âm nhạc (Editorial Layout)**: Bìa album lớn, typography thanh lịch, hình nền đổ bóng mờ theo tông màu album (Ambient Backdrop Blur).
2. **Minh bạch âm thanh tuyệt đối (Audiophile Transparency):**
   * Học tập tính năng biểu tượng **Signal Path** của Roon: Người dùng luôn biết chính xác từng mắt xích trong chuỗi âm thanh (Từ file Google Drive $\rightarrow$ Giải mã $\rightarrow$ Giao thức xuất $\rightarrow$ Thiết bị DAC/Tai nghe).
3. **Thao tác một tay mượt mà (Mobile One-Hand Ergonomics - Roon ARC):**
   * Trên Android: Toàn bộ nút bấm quan trọng (Play/Pause, Seek, Chọn Output, Filter) nằm ở nửa dưới màn hình để dễ dàng thao tác bằng ngón cái.

---

## II. HỆ THỐNG DESIGN SYSTEM & VISUAL TOKENS

### 1. Bảng màu (Color Palette - Dark Luxury)
* **Nền chính (Background):** `#0B0E14` (Obsidian Black sâu thẳm, tiết kiệm pin màn OLED).
* **Bề mặt Card (Card Surface):** `#161B22` (Xám than trầm, phân tách khối nội dung).
* **Đường viền/Ngăn cách (Borders):** `#21262D` (Tối giản, tinh tế).
* **Màu chữ chính (Text Primary):** `#F0F6FC` (Trắng ngà dịu mắt).
* **Màu chữ phụ/Metadata (Text Secondary):** `#8B949E` (Xám tro ghi chú bit-rate, năm phát hành).
* **Màu nhận diện âm thanh (Audiophile Accent Colors):**
  * 🟣 **Bit-Perfect Purple (`#9D4EDD`):** Đại diện cho tín hiệu nguyên bản 100% không suy hao (chuẩn Roon).
  * 🟡 **Hi-Res Gold (`#F59E0B`):** Huy hiệu cho nhạc 24-bit Hi-Res.
  * 🟢 **Standard Lossless (`#10B981`):** Huy hiệu cho FLAC 16-bit / 44.1kHz.
  * 🔵 **Active Play Blue (`#3B82F6`):** Trạng thái đang phát và tiến trình bài hát.

### 2. Typography
* **Font chữ đề xuất:** `Inter` hoặc `Plus Jakarta Sans` (Hỗ trợ tiếng Việt đầy đủ, nét chữ hiện đại, rõ ràng khi hiển thị thông số kỹ thuật).

---

## III. KIẾN TRÚC ĐIỀU HƯỚNG THEO NỀN TẢNG

```
          [ GIAO DIỆN WINDOWS ]                           [ GIAO DIỆN ANDROID ]
┌──────────┬─────────────────────────────┐        ┌─────────────────────────────┐
│ SIDEBAR  │ HEADER & FOCUS FILTER BAR   │        │ [Avatar]   DISCOVER   [🔎]   │
│          ├─────────────────────────────┤        ├─────────────────────────────┤
│ 🏠 Home  │                             │        │   HERO CARD: RECENT ALBUM   │
│ 📚 Kho   │   GRID NGHỆ SĨ / ALBUM      │        │                             │
│ 📁 Drive │   (Dạng thẻ Card lớn)       │        │   HI-RES HIGHLIGHTS         │
│ ⚙ Cài đặt│                             │        │   [Card] [Card] [Card]      │
├──────────┴─────────────────────────────┤        ├─────────────────────────────┤
│ PERSISTENT BOTTOM PLAYER BAR           │        │ 🎵 MINI-PLAYER TRƯỢT        │
│ [Bìa] Tên bài - Ca sĩ [▶] [Waveform] [DAC]      ├─────────────────────────────┤
└────────────────────────────────────────┘        │ 🏠 Home  |  📚 Kho  |  ⚙ Cài │
                                                  └─────────────────────────────┘
```

---

## IV. THIẾT KẾ CHI TIẾT CÁC MÀN HÌNH CHÍNH (HỌC TỪ ROON)

### 1. Màn hình Khám phá & Trang chủ (Home / Discover)
* **Header chào mừng:** Lời chào cá nhân hóa + Dung lượng bộ đệm RAM đang dùng + Nút đồng bộ nhanh Drive (`Sync`).
* **Hàng "Đĩa nhạc mới thêm vào Drive" (Recently Added):** Hiển thị dạng băng chuyền ngang (Horizontal Carousel) các album mới tải lên Google Drive.
* **Bộ sưu tập Hi-Res (Hi-Res Spotlight):** Tự động gom các album 24-bit có dải động cao để người dùng tiện nghe thẩm âm với DAC.
* **Vòng tròn Nghệ sĩ (Artist Bubbles):** Ảnh đại diện tròn của các ca sĩ/ban nhạc có nhiều bài nhất trong Drive.

---

### 2. Bộ lọc thông minh "Focus Filter" (Lấy cảm hứng từ Roon Focus)
Thay vì thanh tìm kiếm đơn điệu, trên đầu danh sách bài hát có hàng **Filter Chips một chạm**:
* **Nguồn lưu trữ (Storage Source):**
  * `[Tất cả Kho Nhạc]` | `[💾 Nhạc trên máy (Local Offline)]` | `[☁️ Nhạc Google Drive]`
* **Định dạng âm thanh (Audio Formats):**
  * `[🟣 Hi-Res 24-bit]` $\rightarrow$ Bấm vào chỉ lọc ra các bài 24-bit/96kHz, 24-bit/192kHz.
  * `[🟢 FLAC Lossless]` $\rightarrow$ Lọc toàn bộ file định dạng FLAC.
  * `[Thập niên 2000s]` | `[Thập niên 1990s]` $\rightarrow$ Lọc theo năm phát hành trong tag.
* **Chế độ xem Cây thư mục:**
  * Cho phép chuyển linh hoạt giữa: *Cây thư mục trên ổ cứng máy* và *Cây thư mục trên Google Drive*.

---

### 3. Màn hình Now Playing (Điểm nhấn đỉnh cao - Roon ARC Style)

```
┌────────────────────────────────────────────────────────┐
│  [⌵ Thu nhỏ]           NOW PLAYING          [⋮ Tùy chọn]│
│                                                        │
│               ┌────────────────────────┐               │
│               │                        │               │
│               │     BÌA ALBUM LỚN      │               │
│               │   (Bo góc 16px, đổ     │               │
│               │    bóng màu nền)       │               │
│               │                        │               │
│               └────────────────────────┘               │
│                                                        │
│   Hotel California (Live On MTV 1994)                  │
│   Eagles • Hell Freezes Over                           │
│                                                        │
│   [ 🟡 HI-RES 24/96 ]   [ 🟣 SIGNAL PATH: BIT-PERFECT ] │  <-- Click mở sơ đồ luồng
│                                                        │
│   | | |||| | ||||||||||||||||||| | |||| | || | ||||    │  <-- Dynamic Waveform Seekbar
│   03:24 ───────────────────────────────────── 07:12    │
│                                                        │
│          [🔀 Shuffle]   [⏮]   [ ▶ ]   [⏭]   [🔁 Loop]   │
│                                                        │
│   Ngõ ra: [ 🎧 Topping DX3 Pro+ (WASAPI Exclusive) ⌵ ]  │  <-- Selector chọn cổng xuất
└────────────────────────────────────────────────────────┘
```

#### Các chi tiết "đắt giá" cần triển khai:
1. **Dynamic Waveform Seekbar:**
   * Thay vì đường kẻ slider mảnh khảnh, thanh tua nhạc vẽ dạng sóng biên độ âm thanh. Nhìn vào biết ngay đoạn điệp khúc cao trào và đoạn intro êm dịu.
2. **Nút "Signal Path" tương tác:**
   * Khi chạm vào huy hiệu `[🟣 SIGNAL PATH]`, một popup/bottom-sheet trượt lên mô phỏng chính xác tính năng Roon Signal Path (xem mục 4).
3. **Selector thiết bị ngõ ra (Output Selector):**
   * Cho phép chạm vào để đổi nhanh: Loa máy tính / Cổng tai nghe 3.5mm / USB DAC / Tai nghe Bluetooth.

---

### 4. Popup Sơ đồ luồng âm thanh (Signal Path Dialog)

Đây là tính năng làm say mê giới Audiophile vì tính minh bạch kỹ thuật:

```
┌────────────────────────────────────────────────────────┐
│                SƠ ĐỒ ÂM THANH (SIGNAL PATH)            │
│                 Trạng thái: 🟣 BIT-PERFECT             │
├────────────────────────────────────────────────────────┤
│                                                        │
│  🟣 NGUỒN PHÁT (SOURCE)                                │
│     Google Drive Cloud Stream                          │
│     FLAC 24-bit / 96kHz • 2 Ch (Stereo) • 3.200 kbps   │
│                  │                                     │
│                  ▼                                     │
│  🟣 BỘ ĐỆM & GIẢI MÃ (CORE ENGINE)                     │
│     media_kit (libmpv C Engine)                        │
│     Đệm RAM: 18.4 MB / 20 MB (Không ghi ổ cứng)        │
│     Giải mã: Native FLAC to 32-bit Float PCM           │
│                  │                                     │
│                  ▼                                     │
│  🟣 XUẤT TÍN HIỆU (OUTPUT PROTOCOL)                    │
│     Windows Audio: WASAPI Exclusive Mode               │
│     Bypass Windows Mixer: 100% Bit-Perfect             │
│                  │                                     │
│                  ▼                                     │
│  🟣 THIẾT BỊ ĐÍCH (ENDPOINT DAC)                       │
│     Khớp tần số: 96.000 Hz / 24-bit                    │
│     Thiết bị: FiiO K5 Pro ESS USB DAC                  │
│                                                        │
└────────────────────────────────────────────────────────┘
```

> **Quy tắc đổi màu trạng thái:**
> * 🟣 **Tím (Bit-Perfect):** Tín hiệu từ file đến tai nghe khớp 100% tần số và bit-depth, không bị resample.
> * 🟢 **Xanh lá (High Quality):** Âm thanh đã qua bộ cân bằng âm lượng (Volume Leveling/EQ).
> * 🟡 **Vàng (Resampled / Bluetooth):** Âm thanh bị nén hoặc downsample (ví dụ xuất qua tai nghe Bluetooth AAC/SBC).

---

### 5. Màn hình Quản lý Thư mục Nguồn (Music Folders Management)
Thiết kế riêng một màn hình trực quan trong Cài đặt để người dùng nắm toàn quyền kiểm soát các nguồn nhạc trong máy:

```
┌────────────────────────────────────────────────────────┐
│  [← Trở lại]       QUẢN LÝ THƯ MỤC NGUỒN               │
├────────────────────────────────────────────────────────┤
│                                                        │
│  THƯ MỤC CỤC BỘ (LOCAL FOLDERS)                        │
│  [ ➕ Thêm thư mục mới ]  [ 🔄 Quét lại toàn bộ ]       │
│                                                        │
│  ┌──────────────────────────────────────────────────┐  │
│  │ 📁 D:\Music\Lossless_FLAC                        │  │
│  │    342 bài hát • 18.4 GB • [🟢 Đang theo dõi]    │  │
│  │    [🔄 Quét lại]  [🗑️ Gỡ bỏ]                     │  │
│  ├──────────────────────────────────────────────────┤  │
│  │ 📁 /storage/emulated/0/Music (Android)           │  │
│  │    128 bài hát • 4.2 GB • [🟢 Đang theo dõi]     │  │
│  │    [🔄 Quét lại]  [🗑️ Gỡ bỏ]                     │  │
│  └──────────────────────────────────────────────────┘  │
│                                                        │
│  TÙY CHỌN NÂNG CAO (SMART FILTERS)                     │
│  [x] Bỏ qua file âm thanh ngắn dưới 30 giây            │
│      (Tránh tiếng chuông báo, voice chat, game SFX)    │
│  [x] Tự động cập nhật khi có file mới (Auto-watch)     │
│                                                        │
└────────────────────────────────────────────────────────┘
```

---

## V. MICRO-INTERACTIONS & TRẢI NGHIỆM THÍNH GIÁC

1. **Cử chỉ vuốt chuyển bài (Swipe Gestures):**
   * Vuốt ngang trên ảnh bìa album để chuyển bài trước/sau (có hiệu ứng trượt 3D nhẹ).
   * Vuốt xuống để thu nhỏ màn hình Now Playing thành thanh Mini-Player.
2. **Haptic Feedback (Rung phản hồi trên Android):**
   * Rung nhẹ (Light click) khi bấm Play/Pause hoặc khi tua sóng âm chạm mốc đánh dấu.
3. **Chuyển bài không tiếng ồn (Gapless Playback & Crossfade nhẹ):**
   * Tự động nối đuôi bài hát mượt mà (Gapless) khi nghe các album nhạc giao hưởng hoặc live concert, không bị ngắt tiếng 1 giây khó chịu giữa 2 bài.
4. **Bảo vệ thính giác khi đổi ngõ ra:**
   * Nếu đang nghe loa ngoài với âm lượng 100% mà người dùng cắm tai nghe có dây/DAC vào, app tự động hạ âm lượng về mức an toàn (40%) để không gây giật mình.
